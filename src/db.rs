use sqlx::{Pool, Row, Sqlite, SqlitePool};

use super::config::Config;
use super::models::ReviewResult;

pub async fn create_db(config: &Config) -> Result<Pool<Sqlite>, sqlx::Error> {
    let pool = SqlitePool::connect(&config.db.url).await?;
    sqlx::query(
        r#"CREATE TABLE IF NOT EXISTS students (
            id INTEGER PRIMARY KEY AUTOINCREMENT,
            name TEXT NOT NULL,
            surname TEXT NOT NULL,
            github TEXT NOT NULL
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
    Ok(pool)
}

pub async fn review_exists(
    pool: &Pool<Sqlite>,
    repository: &str,
    pr_number: u64,
    commit_sha: &str,
) -> Result<bool, sqlx::Error> {
    let count: i64 = sqlx::query_scalar(
        "SELECT COUNT(*) FROM reviews WHERE repository = ? AND pr_number = ? AND commit_sha = ?",
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

pub async fn insert_student(
    pool: &Pool<Sqlite>,
    name: &str,
    surname: &str,
    github: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query(r#"INSERT INTO students (name, surname, github) VALUES (?, ?, ?);"#)
        .bind(name)
        .bind(surname)
        .bind(github)
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
