use std::time::Duration;

use sqlx::{Pool, Sqlite};
use thiserror::Error;
use tracing::{error, info, warn};

use super::config::Config;
use super::db;
use super::github::GitHubClient;
use super::reviewer::Reviewer;
use super::runner;

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
    #[error("runner error: {0}")]
    Runner(#[from] super::runner::RunnerError),
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
        info!(
            interval_secs = self.config.app.poll_interval_secs,
            "application started"
        );
        loop {
            if let Err(error) = self.process_once().await {
                error!(%error, "poll failed");
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
        info!(
            organization,
            count = repositories.len(),
            "repositories loaded"
        );

        for github_repository in repositories {
            let Some((owner, repo)) = github_repository.full_name.split_once('/') else {
                warn!(repository = %github_repository.full_name, "skipping repository with invalid full name");
                continue;
            };

            db::sync_repository(&self.pool, &github_repository.full_name).await?;

            if let Err(error) = self.process_repository(owner, repo).await {
                error!(repository = %format_args!("{owner}/{repo}"), %error, "repository processing failed");
            }
        }
        Ok(())
    }

    async fn process_repository(&self, owner: &str, repo: &str) -> Result<(), AppError> {
        let repository = format!("{owner}/{repo}");
        let pull_requests = self.github.list_open_pull_requests(owner, repo).await?;
        info!(%repository, count = pull_requests.len(), "open pull requests loaded");

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
                error!(
                    %repository,
                    pr = listed_pr.number,
                    %error,
                    "pull request processing failed"
                );
                db::save_review(
                    &self.pool,
                    &repository,
                    listed_pr.number,
                    &listed_pr.head.sha,
                    "failed",
                    None,
                )
                .await?;
                self.save_submission(
                    &listed_pr.user.login,
                    &repository,
                    &listed_pr.head.sha,
                    "failed",
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
        info!(%repository, pr = number, file_count = files.len(), "starting pull request review");
        let source_repository = pr
            .head
            .repo
            .as_ref()
            .map(|repository| repository.full_name.as_str())
            .unwrap_or(repository);
        let sources = runner::clone_and_read_sources(
            &self.config.github.token,
            source_repository,
            &pr.head.sha,
            &files,
        )
        .await?;
        let result = self.reviewer.review(&pr, &files, &sources).await?;
        self.github
            .submit_review(owner, repo, number, &result, &pr.head.sha)
            .await?;
        db::save_review(
            &self.pool,
            repository,
            number,
            &pr.head.sha,
            "published",
            Some(&result),
        )
        .await?;
        self.save_submission(&pr.user.login, repository, &pr.head.sha, "published")
            .await?;
        info!(%repository, pr = number, "pull request reviewed and published");
        Ok(())
    }

    async fn save_submission(
        &self,
        github: &str,
        repository: &str,
        commit_sha: &str,
        status: &str,
    ) -> Result<(), AppError> {
        let Some((student_id, assignment_id)) =
            db::find_student_assignment(&self.pool, github, repository).await?
        else {
            warn!(
                %github,
                %repository,
                "submission not recorded: student/assignment mapping not found"
            );
            return Ok(());
        };

        db::add_submission(&self.pool, student_id, assignment_id, commit_sha, status).await?;
        Ok(())
    }
}
