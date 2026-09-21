use reqwest::Client;
use std::time::Duration;
use tokio::time::sleep;
use tracing::{error, warn};

use super::models::GithubRepo;
use super::models::{PullRequest, PullRequestFile, ReviewDecision, ReviewResult};

#[derive(Debug, Clone)]
pub struct GitHubClient {
    client: Client,
    token: String,
}

impl GitHubClient {
    pub fn new(token: String) -> Result<Self, reqwest::Error> {
        let client = Client::builder()
            .user_agent("corrode-review-bot")
            .connect_timeout(Duration::from_secs(10))
            .timeout(Duration::from_secs(30))
            .build()?;
        Ok(Self { client, token })
    }

    pub async fn list_open_pull_requests(
        &self,
        owner: &str,
        repo: &str,
    ) -> Result<Vec<PullRequest>, reqwest::Error> {
        let mut pull_requests = Vec::new();
        let mut page = 1;
        loop {
            let url = format!(
                "https://api.github.com/repos/{owner}/{repo}/pulls?state=open&per_page=100&page={page}"
            );
            let page_pull_requests: Vec<PullRequest> = self
                .send_get(&url)
                .await?
                .error_for_status()?
                .json()
                .await?;
            let page_size = page_pull_requests.len();
            pull_requests.extend(page_pull_requests);
            if page_size < 100 {
                break;
            }
            page += 1;
        }
        Ok(pull_requests)
    }

    pub async fn list_organization_repositories(
        &self,
        organization: &str,
    ) -> Result<Vec<GithubRepo>, reqwest::Error> {
        let mut repositories = Vec::new();
        let mut page = 1;

        loop {
            let url = format!(
                "https://api.github.com/orgs/{organization}/repos?type=all&per_page=100&page={page}"
            );
            let page_repositories: Vec<GithubRepo> = self
                .send_get(&url)
                .await?
                .error_for_status()?
                .json()
                .await?;
            let page_size = page_repositories.len();
            repositories.extend(page_repositories);

            if page_size < 100 {
                break;
            }
            page += 1;
        }

        Ok(repositories)
    }

    pub async fn fetch_pr_info(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<PullRequest, reqwest::Error> {
        let url = format!("https://api.github.com/repos/{owner}/{repo}/pulls/{pr_number}");
        self.send_get(&url).await?.error_for_status()?.json().await
    }

    pub async fn fetch_pr_files(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
    ) -> Result<Vec<PullRequestFile>, reqwest::Error> {
        let mut files = Vec::new();
        let mut page = 1;
        loop {
            let url = format!(
                "https://api.github.com/repos/{owner}/{repo}/pulls/{pr_number}/files?per_page=100&page={page}"
            );
            let page_files: Vec<PullRequestFile> = self
                .send_get(&url)
                .await?
                .error_for_status()?
                .json()
                .await?;
            let page_size = page_files.len();
            files.extend(page_files);
            if page_size < 100 {
                break;
            }
            page += 1;
        }
        Ok(files)
    }

    pub async fn submit_review(
        &self,
        owner: &str,
        repo: &str,
        pr_number: u64,
        review: &ReviewResult,
        commit_sha: &str,
    ) -> Result<(), reqwest::Error> {
        let url = format!("https://api.github.com/repos/{owner}/{repo}/pulls/{pr_number}/reviews");
        let event = match review.decision {
            ReviewDecision::Approve => "APPROVE",
            ReviewDecision::RequestChanges => "REQUEST_CHANGES",
            ReviewDecision::Comment => "COMMENT",
        };
        let body = ReviewBody {
            body: format_review_body(review),
            event,
            commit_id: commit_sha,
            comments: review
                .comments
                .iter()
                .map(|comment| InlineComment {
                    path: comment.file.clone(),
                    line: comment.line,
                    side: "RIGHT",
                    body: comment.body.clone(),
                })
                .collect(),
        };
        let response = self
            .client
            .post(url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await?;
        if response.status().is_success() {
            return Ok(());
        }

        let response_error = response.error_for_status_ref().unwrap_err();
        let response_body = response.text().await.unwrap_or_default();
        error!(
            status = %response_error,
            body = %response_body,
            "GitHub review request rejected"
        );

        // GitHub rejects the whole review when a model points to a line outside the diff.
        // Preserve the review by retrying it as a summary-only review.
        if response_error.status() == Some(reqwest::StatusCode::UNPROCESSABLE_ENTITY)
            && !body.comments.is_empty()
        {
            let fallback = ReviewBody {
                comments: Vec::new(),
                ..body
            };
            self.client
                .post(format!(
                    "https://api.github.com/repos/{owner}/{repo}/pulls/{pr_number}/reviews"
                ))
                .headers(self.headers())
                .json(&fallback)
                .send()
                .await?
                .error_for_status()?;
            return Ok(());
        }

        Err(response_error)
    }

    fn headers(&self) -> reqwest::header::HeaderMap {
        let mut headers = reqwest::header::HeaderMap::new();
        headers.insert(
            reqwest::header::AUTHORIZATION,
            format!("Bearer {}", self.token)
                .parse()
                .expect("valid auth header"),
        );
        headers.insert(
            reqwest::header::ACCEPT,
            "application/vnd.github+json"
                .parse()
                .expect("valid accept header"),
        );
        headers
    }

    async fn send_get(&self, url: &str) -> Result<reqwest::Response, reqwest::Error> {
        let attempts = 3;
        for attempt in 1..=attempts {
            match self.client.get(url).headers(self.headers()).send().await {
                Ok(response) => return Ok(response),
                Err(error) if attempt < attempts => {
                    warn!(
                        attempt,
                        attempts,
                        %url,
                        %error,
                        "GitHub GET failed, retrying"
                    );
                    sleep(Duration::from_secs(attempt as u64 * 2)).await;
                }
                Err(error) => return Err(error),
            }
        }
        unreachable!()
    }
}

#[derive(serde::Serialize)]
struct ReviewBody<'a> {
    body: String,
    event: &'a str,
    commit_id: &'a str,
    comments: Vec<InlineComment>,
}

#[derive(serde::Serialize)]
struct InlineComment {
    path: String,
    line: u64,
    side: &'static str,
    body: String,
}

fn format_review_body(review: &ReviewResult) -> String {
    let mut body = format!("## Corrode review\n\n{}", review.summary);
    for comment in &review.comments {
        body.push_str(&format!(
            "\n\n- **{}:{} ({:?})** {}",
            comment.file, comment.line, comment.severity, comment.body
        ));
    }
    body
}
