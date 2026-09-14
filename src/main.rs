mod config;
mod db;
mod github;
mod models;
mod reviewer;
mod runner;

#[tokio::main]
async fn main() {
    let config = config::Config::load().expect("Failed to load configuration");

    let client = github::GitHubClient::new("YOUR_GITHUB_TOKEN".to_string());
}
