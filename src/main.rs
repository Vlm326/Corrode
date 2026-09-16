use tokio;

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
    if let Err(error) = run().await {
        eprintln!("fatal error: {error}");
        std::process::exit(1);
    }
}

async fn run() -> Result<(), Box<dyn std::error::Error>> {
    let config = config::Config::load()?;
    let pool = db::create_db(&config).await?;
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
