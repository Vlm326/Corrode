use reqwest::Client;

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
        let url =
            format!("https://api.github.com/repos/{owner}/{repo}/pulls?state=open&per_page=100");
        self.client
            .get(url)
            .headers(self.headers())
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
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
        let url = format!("https://api.github.com/repos/{owner}/{repo}/pulls/{pr_number}/files");
        let url = format!("{url}?per_page=100");
        self.client
            .get(url)
            .headers(self.headers())
            .send()
            .await?
            .error_for_status()?
            .json()
            .await
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
