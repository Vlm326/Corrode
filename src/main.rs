use tokio;
use tracing::{error, info};
use tracing_subscriber::EnvFilter;

mod app;
mod config;
mod csv;
mod db;
mod github;
mod models;
mod reviewer;
mod runner;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .init();

    if let Err(error) = run().await {
        error!(%error, "fatal error");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = config::Config::load()?;
    info!("starting Corrode");
    let pool = db::create_db(&config).await?;
    let students = csv::parse_students(&config.db.students_csv)?;
    info!(count = students.len(), path = %config.db.students_csv, "loaded students from CSV");
    db::import_students(&pool, &students).await?;
    info!("student data imported");
    let github = github::GitHubClient::new(config.github.token.clone())?;
    let reviewer = reviewer::Reviewer::new(
        config.openai.api_key.clone(),
        config.openai.model.clone(),
        config.openai.base_url.clone(),
    )?;
    let application = app::Application::new(config, pool, github, reviewer);
    application.run().await?;
    Ok(())
}
