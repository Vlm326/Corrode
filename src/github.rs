use reqwest::Client;

use super::models::GithubRepo;
use super::models::{PullRequest, PullRequestFile, ReviewDecision, ReviewResult};

#[derive(Debug, Clone)]
pub struct GitHubClient {
    client: Client,
    token: String,
}

impl GitHubClient {
    pub fn new(token: String) -> Result<Self, reqwest::Error> {
        let client = Client::builder().user_agent("corrode-review-bot").build()?;
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
                .client
                .get(url)
                .headers(self.headers())
                .send()
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
                .client
                .get(url)
                .headers(self.headers())
                .send()
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
        self.client
            .get(url)
            .headers(self.headers())
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
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
                .client
                .get(url)
                .headers(self.headers())
                .send()
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
        };
        self.client
            .post(url)
            .headers(self.headers())
            .json(&body)
            .send()
            .await?
            .error_for_status()?;
        Ok(())
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
}

#[derive(serde::Serialize)]
struct ReviewBody<'a> {
    body: String,
    event: &'a str,
    commit_id: &'a str,
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
