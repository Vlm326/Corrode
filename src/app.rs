use std::time::Duration;

use sqlx::{Pool, Sqlite};
use thiserror::Error;

use super::config::Config;
use super::db;
use super::github::GitHubClient;
use super::reviewer::Reviewer;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("invalid GitHub repository configuration: {0}")]
    Repository(String),
    #[error("database error: {0}")]
    Database(#[from] sqlx::Error),
    #[error("GitHub error: {0}")]
    Github(#[from] reqwest::Error),
    #[error("reviewer error: {0}")]
    Reviewer(#[from] super::reviewer::ReviewerError),
}

pub struct Application {
    config: Config,
    pool: Pool<Sqlite>,
    github: GitHubClient,
    reviewer: Reviewer,
}

impl Application {
    pub fn new(
        config: Config,
        pool: Pool<Sqlite>,
        github: GitHubClient,
        reviewer: Reviewer,
    ) -> Self {
        Self {
            config,
            pool,
            github,
            reviewer,
        }
    }

    pub async fn run(&self) -> Result<(), AppError> {
        loop {
            if let Err(error) = self.process_once().await {
                eprintln!("poll failed: {error}");
            }
            tokio::time::sleep(Duration::from_secs(self.config.app.poll_interval_secs)).await;
        }
    }

    async fn process_once(&self) -> Result<(), AppError> {
        let organization = self
            .config
            .github_organization()
            .map_err(AppError::Repository)?;
        let repositories = self
            .github
            .list_organization_repositories(organization)
            .await?;

        for github_repository in repositories {
            let Some((owner, repo)) = github_repository.full_name.split_once('/') else {
                eprintln!(
                    "skipping repository with invalid full name: {}",
                    github_repository.full_name
                );
                continue;
            };

            if let Err(error) = self.process_repository(owner, repo).await {
                eprintln!("repository {}/{} failed: {error}", owner, repo);
            }
        }
        Ok(())
    }

    async fn process_repository(&self, owner: &str, repo: &str) -> Result<(), AppError> {
        let repository = format!("{owner}/{repo}");
        let pull_requests = self.github.list_open_pull_requests(owner, repo).await?;

        for listed_pr in pull_requests {
            if listed_pr.draft
                || db::review_exists(
                    &self.pool,
                    &repository,
                    listed_pr.number,
                    &listed_pr.head.sha,
                )
                .await?
            {
                continue;
            }

            if let Err(error) = self
                .process_pr(owner, repo, &repository, listed_pr.number)
                .await
            {
                eprintln!("{repository} PR #{} failed: {error}", listed_pr.number);
                db::save_review(
                    &self.pool,
                    &repository,
                    listed_pr.number,
                    &listed_pr.head.sha,
                    "failed",
                    None,
                )
                .await?;
            }
        }
        Ok(())
    }

    async fn process_pr(
        &self,
        owner: &str,
        repo: &str,
        repository: &str,
        number: u64,
    ) -> Result<(), AppError> {
        let pr = self.github.fetch_pr_info(owner, repo, number).await?;
        if pr.state != "open" || pr.draft {
            return Ok(());
        }
        let files = self.github.fetch_pr_files(owner, repo, number).await?;
        let result = self.reviewer.review(&pr, &files).await?;
        self.github
            .submit_review(owner, repo, number, &result, &pr.head.sha)
            .await?;
        db::save_review(
            &self.pool,
            repository,
            number,
            &pr.head.sha,
            "reviewed",
            Some(&result),
        )
        .await?;
        println!("Reviewed {repository}#{number}");
        Ok(())
    }
}
