use sqlx::{Pool, Row, Sqlite, SqlitePool};

use super::config::Config;
use super::csv::StudentRow;
use super::models::ReviewResult;

pub async fn create_db(config: &Config) -> Result<Pool<Sqlite>, sqlx::Error> {
    let options = config
        .db
        .url
        .parse::<sqlx::sqlite::SqliteConnectOptions>()?
        .create_if_missing(true);
    let pool = SqlitePool::connect_with(options).await?;
    sqlx::query("PRAGMA foreign_keys = ON")
        .execute(&pool)
        .await?;
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS students (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            surname TEXT NOT NULL,
            github TEXT NOT NULL UNIQUE
        );"#,
    )
    .execute(&pool)
    .await?;
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS reviews (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            repository TEXT NOT NULL,
            pr_number INTEGER NOT NULL,
            commit_sha TEXT NOT NULL,
            status TEXT NOT NULL,
            result TEXT,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(repository, pr_number, commit_sha)
        );"#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS assignments (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            UNIQUE(name)
        );"#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS student_repositories (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            student_id INTEGER NOT NULL,
            assignment_id INTEGER NOT NULL,
            repository TEXT NOT NULL UNIQUE,
            FOREIGN KEY(student_id) REFERENCES students(id),
            FOREIGN KEY(assignment_id) REFERENCES assignments(id),
            UNIQUE(student_id, assignment_id)
        );"#,
    )
    .execute(&pool)
    .await?;

    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS submissions (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            student_id INTEGER NOT NULL,
            assignment_id INTEGER NOT NULL,
            commit_sha TEXT NOT NULL,
            status TEXT NOT NULL,
            created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
            UNIQUE(student_id, assignment_id, commit_sha),
            FOREIGN KEY(student_id) REFERENCES students(id),
            FOREIGN KEY(assignment_id) REFERENCES assignments(id)
        );"#,
    )
    .execute(&pool)
    .await?;
    Ok(pool)
}

pub async fn import_students(
    pool: &Pool<Sqlite>,
    students: &[StudentRow],
) -> Result<(), sqlx::Error> {
    for student in students {
        sqlx::query(
            "INSERT INTO students (name, surname, github) VALUES (?, ?, ?) ON CONFLICT(github) DO UPDATE SET name = excluded.name, surname = excluded.surname",
        )
        .bind(&student.name)
        .bind(&student.surname)
        .bind(&student.github)
        .execute(pool)
        .await?;
    }
    Ok(())
}

pub async fn sync_repository(
    pool: &Pool<Sqlite>,
    full_repository: &str,
) -> Result<(), sqlx::Error> {
    let repository_name = full_repository
        .rsplit('/')
        .next()
        .unwrap_or(full_repository);
    let students = sqlx::query("SELECT id, github FROM students")
        .fetch_all(pool)
        .await?;

    for student in students {
        let student_id: i64 = student.get("id");
        let github: String = student.get("github");
        let suffix = format!("-{github}");
        if !repository_name
            .to_ascii_lowercase()
            .ends_with(&suffix.to_ascii_lowercase())
        {
            continue;
        }

        let assignment_name =
            repository_name[..repository_name.len() - suffix.len()].trim_end_matches('-');
        if assignment_name.is_empty() {
            continue;
        }

        sqlx::query("INSERT OR IGNORE INTO assignments (name) VALUES (?)")
            .bind(assignment_name)
            .execute(pool)
            .await?;
        let assignment_id: i64 = sqlx::query_scalar("SELECT id FROM assignments WHERE name = ?")
            .bind(assignment_name)
            .fetch_one(pool)
            .await?;

        sqlx::query(
            "INSERT INTO student_repositories (student_id, assignment_id, repository) VALUES (?, ?, ?) ON CONFLICT(repository) DO UPDATE SET student_id = excluded.student_id, assignment_id = excluded.assignment_id",
        )
        .bind(student_id)
        .bind(assignment_id)
        .bind(full_repository)
        .execute(pool)
        .await?;
        break;
    }

    Ok(())
}

pub async fn add_assignment(pool: &Pool<Sqlite>, name: &str) -> Result<(), sqlx::Error> {
    sqlx::query("INSERT INTO assignments (name) VALUES (?)")
        .bind(name)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn add_student_repository(
    pool: &Pool<Sqlite>,
    student_id: i64,
    assignment_id: i64,
    repository: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO student_repositories (student_id, assignment_id, repository) VALUES (?, ?, ?)",
    )
    .bind(student_id)
    .bind(assignment_id)
    .bind(repository)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn add_submission(
    pool: &Pool<Sqlite>,
    student_id: i64,
    assignment_id: i64,
    commit_sha: &str,
    status: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO submissions (student_id, assignment_id, commit_sha, status) VALUES (?, ?, ?, ?) ON CONFLICT(student_id, assignment_id, commit_sha) DO UPDATE SET status = excluded.status",
    )
        .bind(student_id)
        .bind(assignment_id)
        .bind(commit_sha)
        .bind(status)
        .execute(pool)
        .await?;
    Ok(())
}

pub async fn find_student_assignment(
    pool: &Pool<Sqlite>,
    github: &str,
    repository: &str,
) -> Result<Option<(i64, i64)>, sqlx::Error> {
    sqlx::query_as(
        "SELECT student_repositories.student_id, student_repositories.assignment_id FROM student_repositories JOIN students ON students.id = student_repositories.student_id WHERE students.github = ? AND student_repositories.repository = ?",
    )
    .bind(github)
    .bind(repository)
    .fetch_optional(pool)
    .await
}

pub async fn get_all_submissions_by_student(
    pool: &Pool<Sqlite>,
    student_id: i64,
) -> Result<Vec<(String, String, String, String)>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT assignments.name, submissions.commit_sha, submissions.status, submissions.created_at FROM submissions JOIN assignments ON assignments.id = submissions.assignment_id WHERE submissions.student_id = ? ORDER BY submissions.created_at DESC",
    )
    .bind(student_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| {
            (
                row.get("name"),
                row.get("commit_sha"),
                row.get("status"),
                row.get("created_at"),
            )
        })
        .collect())
}

pub async fn review_exists(
    pool: &Pool<Sqlite>,
    repository: &str,
    pr_number: u64,
    commit_sha: &str,
) -> Result<bool, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM reviews WHERE repository = ? AND pr_number = ? AND commit_sha = ? AND status IN ('reviewed', 'published')",
    )
    .bind(repository)
    .bind(pr_number as i64)
    .bind(commit_sha)
    .fetch_one(pool)
    .await?;
    Ok(count > 0)
}

pub async fn save_review(
    pool: &Pool<Sqlite>,
    repository: &str,
    pr_number: u64,
    commit_sha: &str,
    status: &str,
    result: Option<&ReviewResult>,
) -> Result<(), sqlx::Error> {
    let result = result
        .map(serde_json::to_string)
        .transpose()
        .map_err(|error| sqlx::Error::Protocol(format!("failed to serialize review: {error}")))?;
    sqlx::query(
        "INSERT OR REPLACE INTO reviews (repository, pr_number, commit_sha, status, result) VALUES (?, ?, ?, ?, ?)",
    )
    .bind(repository)
    .bind(pr_number as i64)
    .bind(commit_sha)
    .bind(status)
    .bind(result)
    .execute(pool)
    .await?;
    Ok(())
}

pub async fn get_students(
    pool: &Pool<Sqlite>,
) -> Result<Vec<(i64, String, String, String)>, sqlx::Error> {
    let rows = sqlx::query(r#"SELECT id, name, surname, github FROM students;"#)
        .fetch_all(pool)
        .await?;
    let students = rows
        .into_iter()
        .map(|row| {
            (
                row.get::<i64, _>("id"),
                row.get::<String, _>("name"),
                row.get::<String, _>("surname"),
                row.get::<String, _>("github"),
            )
        })
        .collect();
    Ok(students)
}

pub async fn delete_student(pool: &Pool<Sqlite>, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query(r#"DELETE FROM students WHERE id = ?;"#)
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
