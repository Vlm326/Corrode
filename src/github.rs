use super::models::PullRequest;
use reqwest::Client;

#[derive(Debug, Clone)]
pub struct GitHubClient {
    client: Client,
    token: String,
}

impl GitHubClient {
    pub fn new(token: String) -> Self {
        let client = reqwest::Client::builder()
            .user_agent("Corrode review tool")
            .build()
            .expect("failed to build HTTP client");
        Self { client, token }
    }

    pub async fn fetch_pr_info(
        &self,
        owner: String,
        repo: String,
        pr_number: u64,
    ) -> Result<PullRequest, Box<dyn std::error::Error>> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}",
            owner, repo, pr_number
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?
            .error_for_status()?;
        let pr_info: PullRequest = response.json().await?;

        Ok(pr_info)
    }
    pub async fn fetch_pr_diff(
        &self,
        owner: String,
        repo: String,
        pr_number: u64,
    ) -> Result<String, Box<dyn std::error::Error>> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}",
            owner, repo, pr_number
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .header("Accept", "application/vnd.github.v3.diff")
            .send()
            .await?
            .error_for_status()?;
        let diff_text = response.text().await?;

        Ok(diff_text)
    }

    pub async fn fetch_pr_files(
        &self,
        owner: String,
        repo: String,
        pr_number: u64,
    ) -> Result<Vec<String>, Box<dyn std::error::Error>> {
        let url = format!(
            "https://api.github.com/repos/{}/{}/pulls/{}/files",
            owner, repo, pr_number
        );

        let response = self
            .client
            .get(&url)
            .header("Authorization", format!("Bearer {}", self.token))
            .send()
            .await?
            .error_for_status()?;
        let files: Vec<serde_json::Value> = response.json().await?;

        let file_names = files
            .into_iter()
            .filter_map(|file| {
                file.get("filename")
                    .and_then(|f| f.as_str())
                    .map(String::from)
            })
            .collect();

        Ok(file_names)
    }
    pub async fn clone_repo(&self, owner: String, repo: String) {
        let url = format!("git@github.com:{}/{}.git", owner, repo);
        let output = tokio::process::Command::new("git")
            .arg("clone")
            .arg(url)
            .output()
            .await
            .expect("failed to execute git clone command");
    }

}
