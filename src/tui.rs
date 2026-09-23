use std::io::{self, IsTerminal, Write};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use crossterm::cursor::{Hide, MoveTo, Show};
use crossterm::event::{self, Event, KeyCode, KeyEvent};
use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal::{self, Clear, ClearType, EnterAlternateScreen, LeaveAlternateScreen};
use crossterm::{execute, queue};
use sqlx::{Pool, Sqlite};
use tracing::error;

use super::app::Application;
use super::config::Config;
use super::csv;
use super::db;

const LOGO: &str = include_str!("../logo.txt");

enum Screen {
    Main,
    Students,
    Database((i64, i64, i64, i64)),
    Settings,
    Message(String),
}

pub async fn run(
    application: Application,
    pool: Pool<Sqlite>,
    config: Config,
) -> Result<(), Box<dyn std::error::Error>> {
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err(io::Error::new(
            io::ErrorKind::NotConnected,
            "TUI requires an interactive terminal; run cargo run from a terminal, not a pipe or background task",
        )
        .into());
    }
    let mut guard = TerminalGuard::enter()?;

    let result = run_loop(guard.stdout_mut(), application, &pool, &config).await;

    result
}

struct TerminalGuard {
    stdout: io::Stdout,
    active: bool,
}

impl TerminalGuard {
    fn enter() -> io::Result<Self> {
        terminal::enable_raw_mode()?;
        let mut stdout = io::stdout();
        if let Err(error) = execute!(stdout, EnterAlternateScreen, Hide) {
            let _ = terminal::disable_raw_mode();
            return Err(error);
        }
        Ok(Self {
            stdout,
            active: true,
        })
    }

    fn stdout_mut(&mut self) -> &mut io::Stdout {
        &mut self.stdout
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        if !self.active {
            return;
        }
        let _ = execute!(self.stdout, Show, LeaveAlternateScreen);
        let _ = terminal::disable_raw_mode();
        self.active = false;
    }
}

async fn run_loop(
    stdout: &mut io::Stdout,
    mut application: Application,
    pool: &Pool<Sqlite>,
    config: &Config,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut screen = Screen::Main;
    let mut menu_index = 0;
    let mut student_index = 0;
    let mut students = db::get_students(pool).await?;

    loop {
        draw(
            stdout,
            &screen,
            menu_index,
            student_index,
            &students,
            config,
        )?;

        if !event::poll(Duration::from_millis(100))? {
            continue;
        }
        let Event::Key(KeyEvent { code, .. }) = event::read()? else {
            continue;
        };

        match &mut screen {
            Screen::Main => match code {
                KeyCode::Char('q') => return Ok(()),
                KeyCode::Char('j') | KeyCode::Down => menu_index = (menu_index + 1) % 6,
                KeyCode::Char('k') | KeyCode::Up => menu_index = (menu_index + 5) % 6,
                KeyCode::Enter => match menu_index {
                    0 => {
                        application.set_student(None);
                        screen = run_once(stdout, &application, None).await;
                    }
                    1 => {
                        run_continuous(stdout, application, None).await?;
                        return Ok(());
                    }
                    2 => {
                        screen = Screen::Students;
                        student_index = 0;
                    }
                    3 => screen = Screen::Database(db::database_summary(pool).await?),
                    4 => screen = Screen::Settings,
                    5 => return Ok(()),
                    _ => unreachable!(),
                },
                _ => {}
            },
            Screen::Students => match code {
                KeyCode::Esc | KeyCode::Char('h') | KeyCode::Char('q') => screen = Screen::Main,
                KeyCode::Char('j') | KeyCode::Down if !students.is_empty() => {
                    student_index = (student_index + 1) % students.len();
                }
                KeyCode::Char('k') | KeyCode::Up if !students.is_empty() => {
                    student_index = (student_index + students.len() - 1) % students.len();
                }
                KeyCode::Char('r') => students = db::get_students(pool).await?,
                KeyCode::Enter if !students.is_empty() => {
                    let github = students[student_index].3.clone();
                    application.set_student(Some(github));
                    screen = run_once(
                        stdout,
                        &application,
                        Some(students[student_index].3.clone()),
                    )
                    .await;
                    application.set_student(None);
                }
                _ => {}
            },
            Screen::Database(_) => match code {
                KeyCode::Esc | KeyCode::Char('h') | KeyCode::Char('q') => screen = Screen::Main,
                _ => {}
            },
            Screen::Settings => match code {
                KeyCode::Esc | KeyCode::Char('h') | KeyCode::Char('q') => screen = Screen::Main,
                KeyCode::Char('r') => {
                    let imported = csv::parse_students(&config.db.students_csv)?;
                    db::import_students(pool, &imported).await?;
                    students = db::get_students(pool).await?;
                    screen = Screen::Message(format!("reloaded {} students", imported.len()));
                }
                _ => {}
            },
            Screen::Message(_) => match code {
                KeyCode::Esc | KeyCode::Enter | KeyCode::Char('q') => screen = Screen::Main,
                _ => {}
            },
        }
    }
}

async fn run_once(
    stdout: &mut io::Stdout,
    application: &Application,
    student: Option<String>,
) -> Screen {
    let status = application.status_handle();
    let operation = application.process_once();
    tokio::pin!(operation);

    loop {
        tokio::select! {
            result = &mut operation => {
                return match result {
                    Ok(()) => Screen::Message("polling iteration completed".to_string()),
                    Err(error) => {
                        error!(%error, "one-shot polling failed");
                        Screen::Message(format!("polling failed: {error}"))
                    }
                };
            }
            _ = tokio::time::sleep(Duration::from_millis(250)) => {
                let _ = draw_running(stdout, student.as_deref(), &status);
            }
        }
    }
}

async fn run_continuous(
    stdout: &mut io::Stdout,
    mut application: Application,
    student: Option<String>,
) -> Result<(), Box<dyn std::error::Error>> {
    let selected_student = student.clone();
    let status = application.status_handle();
    application.set_student(student);
    tokio::spawn(async move {
        let _ = application.run().await;
    });

    loop {
        draw_running(stdout, selected_student.as_deref(), &status)?;
        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(KeyEvent { code, .. }) = event::read()? {
                if matches!(code, KeyCode::Char('q') | KeyCode::Esc) {
                    return Ok(());
                }
            }
        }
    }
}

fn draw(
    stdout: &mut io::Stdout,
    screen: &Screen,
    menu_index: usize,
    student_index: usize,
    students: &[(i64, String, String, String)],
    config: &Config,
) -> io::Result<()> {
    let (width, height) = terminal::size()?;
    queue!(stdout, Clear(ClearType::All), MoveTo(0, 0))?;
    for line in LOGO.lines() {
        queue!(
            stdout,
            SetForegroundColor(Color::DarkCyan),
            Print(
                wrap_chars(line, width as usize)
                    .into_iter()
                    .map(|line| center_line(&line, width as usize))
                    .collect::<Vec<_>>()
                    .join("\r\n"),
            ),
            Print("\r\n")
        )?;
    }
    queue!(stdout, ResetColor, Print("\r\n"))?;

    match screen {
        Screen::Main => draw_menu(stdout, menu_index, width as usize)?,
        Screen::Students => draw_students(stdout, student_index, students, width as usize)?,
        Screen::Database(stats) => draw_database(stdout, *stats, width as usize)?,
        Screen::Settings => draw_settings(stdout, config, width as usize)?,
        Screen::Message(message) => {
            queue!(
                stdout,
                Print("\r\n"),
                SetForegroundColor(Color::Cyan),
                Print(
                    wrapped_lines(message, width as usize)
                        .into_iter()
                        .map(|line| center_line(&line, width as usize))
                        .collect::<Vec<_>>()
                        .join("\r\n"),
                ),
                ResetColor
            )?;
        }
    }
    queue!(
        stdout,
        MoveTo(0, height.saturating_sub(1)),
        Clear(ClearType::CurrentLine),
        SetForegroundColor(Color::DarkGrey),
        Print(center_line(
            "j/k move  Enter select  Esc back  q quit",
            width as usize,
        )),
        ResetColor
    )?;
    stdout.flush()
}

fn draw_menu(stdout: &mut io::Stdout, selected: usize, width: usize) -> io::Result<()> {
    let items = [
        "Run all repositories once",
        "Run all repositories continuously",
        "Run one student",
        "Database",
        "Settings",
        "Quit",
    ];
    for (index, item) in items.iter().enumerate() {
        for line in wrapped_lines(item, width.saturating_sub(4)) {
            let text = if index == selected {
                format!("> {line}")
            } else {
                format!("  {line}")
            };
            queue!(
                stdout,
                SetAttribute(if index == selected {
                    Attribute::Bold
                } else {
                    Attribute::Reset
                }),
                SetForegroundColor(if index == selected {
                    Color::Cyan
                } else {
                    Color::Grey
                }),
                Print(center_line(&text, width)),
                ResetColor,
                SetAttribute(Attribute::Reset),
                Print("\r\n")
            )?;
        }
    }
    Ok(())
}

fn draw_students(
    stdout: &mut io::Stdout,
    selected: usize,
    students: &[(i64, String, String, String)],
    width: usize,
) -> io::Result<()> {
    queue!(
        stdout,
        SetForegroundColor(Color::Cyan),
        Print(center_line("Select student", width)),
        ResetColor,
        Print("\r\n\r\n")
    )?;
    for (index, student) in students.iter().enumerate() {
        let text = format!("{} {}", student.1, student.2);
        for line in wrapped_lines(&text, width.saturating_sub(4)) {
            let text = if index == selected {
                format!("> {line}")
            } else {
                format!("  {line}")
            };
            queue!(
                stdout,
                SetForegroundColor(if index == selected {
                    Color::Cyan
                } else {
                    Color::Grey
                }),
                Print(center_line(&text, width)),
                ResetColor,
                Print("\r\n")
            )?;
        }
    }
    if students.is_empty() {
        queue!(
            stdout,
            SetForegroundColor(Color::DarkGrey),
            Print(center_line("No students imported", width)),
            ResetColor,
            Print("\r\n")
        )?;
    }
    Ok(())
}

fn draw_database(
    stdout: &mut io::Stdout,
    stats: (i64, i64, i64, i64),
    width: usize,
) -> io::Result<()> {
    queue!(
        stdout,
        SetForegroundColor(Color::Cyan),
        Print(center_line("Database", width)),
        ResetColor,
        Print("\r\n\r\n")
    )?;
    for line in [
        format!("students: {}", stats.0),
        format!("assignments: {}", stats.1),
        format!("repository mappings: {}", stats.2),
        format!("submissions: {}", stats.3),
    ] {
        queue!(stdout, Print(center_line(&line, width)), Print("\r\n"))?;
    }
    queue!(
        stdout,
        Print("\r\n"),
        SetForegroundColor(Color::DarkGrey),
        Print(center_line("Press Esc to return", width)),
        ResetColor,
        Print("\r\n")
    )
}

fn draw_settings(stdout: &mut io::Stdout, config: &Config, width: usize) -> io::Result<()> {
    queue!(
        stdout,
        SetForegroundColor(Color::Cyan),
        Print(center_line("Settings", width)),
        ResetColor,
        Print("\r\n\r\n")
    )?;
    for line in [
        format!("organization: {}", config.github.organization),
        format!("model: {}", config.openai.model),
        format!("students CSV: {}", config.db.students_csv),
        format!("database: {}", config.db.url),
    ] {
        for line in wrapped_lines(&line, width.saturating_sub(4)) {
            queue!(stdout, Print(center_line(&line, width)), Print("\r\n"))?;
        }
    }
    queue!(
        stdout,
        Print("\r\n"),
        SetForegroundColor(Color::DarkGrey),
        Print(center_line("r reload students CSV", width)),
        ResetColor,
        Print("\r\n")
    )
}

fn draw_running(
    stdout: &mut io::Stdout,
    student: Option<&str>,
    status: &Arc<Mutex<String>>,
) -> io::Result<()> {
    let (width, _) = terminal::size()?;
    let status = status
        .lock()
        .map(|status| status.clone())
        .unwrap_or_else(|_| "Выполнение...".to_string());
    queue!(
        stdout,
        Clear(ClearType::All),
        MoveTo(0, 0),
        SetForegroundColor(Color::Cyan),
        Print(center_line("Corrode is running", width as usize)),
        ResetColor,
        Print("\r\n\r\n")
    )?;
    if let Some(student) = student {
        queue!(
            stdout,
            Print(center_line(&format!("Student: {student}"), width as usize)),
            Print("\r\n")
        )?;
    } else {
        queue!(
            stdout,
            Print(center_line("Scope: all repositories", width as usize)),
            Print("\r\n")
        )?;
    }
    queue!(
        stdout,
        SetForegroundColor(Color::DarkGrey),
        Print(center_line(&status, width as usize)),
        ResetColor,
        Print("\r\n")
    )?;
    stdout.flush()
}

fn wrapped_lines(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let mut result = Vec::new();
    for raw_line in text.lines() {
        if raw_line.is_empty() {
            result.push(String::new());
            continue;
        }
        let mut line = String::new();
        for word in raw_line.split_whitespace() {
            if !line.is_empty() && line.chars().count() + 1 + word.chars().count() > width {
                result.push(line);
                line = String::new();
            }
            if word.chars().count() > width {
                if !line.is_empty() {
                    result.push(line);
                    line = String::new();
                }
                let chars: Vec<char> = word.chars().collect();
                for chunk in chars.chunks(width) {
                    result.push(chunk.iter().collect());
                }
            } else {
                if !line.is_empty() {
                    line.push(' ');
                }
                line.push_str(word);
            }
        }
        if !line.is_empty() {
            result.push(line);
        }
    }
    if result.is_empty() {
        result.push(String::new());
    }
    result
}

fn wrap_chars(text: &str, width: usize) -> Vec<String> {
    let width = width.max(1);
    let chars: Vec<char> = text.chars().collect();
    if chars.is_empty() {
        return vec![String::new()];
    }
    chars
        .chunks(width)
        .map(|chunk| chunk.iter().collect())
        .collect()
}

fn center_line(text: &str, width: usize) -> String {
    let width = width.max(1);
    let text_width = text.chars().count().min(width);
    let left_padding = width.saturating_sub(text_width) / 2;
    format!("{}{}", " ".repeat(left_padding), text)
}
