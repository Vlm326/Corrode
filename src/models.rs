use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Debug, Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub response_format: Option<ResponseFormat>,
}

#[derive(Debug, Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Debug, Deserialize)]
pub struct Choice {
    pub index: u64,
    pub message: ResponseMessage,
    pub finish_reason: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct ResponseMessage {
    pub role: String,
    pub content: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

#[derive(Debug, Serialize)]
pub struct ResponseFormat {
    pub r#type: String,
}

pub fn system(content: impl Into<String>) -> Message {
    Message {
        role: "system".to_string(),
        content: content.into(),
    }
}

pub fn user(content: impl Into<String>) -> Message {
    Message {
        role: "user".to_string(),
        content: content.into(),
    }
}

// Github models
#[derive(Debug, Clone, Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub state: String,
    pub draft: bool,
    pub title: String,
    pub body: Option<String>,
    pub user: GithubUser,
    pub head: PullRequestRef,
    pub base: PullRequestRef,
    #[serde(default)]
    pub changed_files: u64,
    #[serde(default)]
    pub additions: u64,
    #[serde(default)]
    pub deletions: u64,
}
#[derive(Debug, Clone, Deserialize)]
pub struct GithubUser {
    pub login: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PullRequestRef {
    pub r#ref: String,
    pub sha: String,
    pub repo: Option<GithubRepo>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GithubRepo {
    pub full_name: String,
}
#[derive(Debug, Clone, Deserialize)]
pub struct PullRequestFile {
    pub filename: String,
    pub status: String,
    pub additions: u64,
    pub deletions: u64,
    pub changes: u64,
    pub patch: Option<String>,
}

// review models

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewResult {
    pub summary: String,
    pub decision: ReviewDecision,
    pub comments: Vec<ReviewComment>,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReviewDecision {
    Approve,
    RequestChanges,
    Comment,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ReviewComment {
    pub file: String,
    pub line: u64,
    pub severity: Severity,
    pub body: String,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Severity {
    Low,
    Medium,
    High,
    Critical,
}
