use sqlx::{Pool, Row, Sqlite, SqlitePool};

use super::config::Config;

pub async fn create_db(config: &Config) -> Result<Pool<Sqlite>, sqlx::Error> {
    let pool = SqlitePool::connect(&config.db_url).await?;
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
    Ok(pool)
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
