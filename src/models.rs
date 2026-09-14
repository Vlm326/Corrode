use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

#[derive(Serialize)]
pub struct ChatRequest {
    pub model: String,
    pub messages: Vec<Message>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub max_tokens: Option<u64>,
}

#[derive(Deserialize)]
pub struct ChatResponse {
    pub id: String,
    pub model: String,
    pub choices: Vec<Choice>,
    pub usage: Option<Usage>,
}

#[derive(Deserialize)]
pub struct Choice {
    pub index: u64,
    pub message: Message,
    pub finish_reason: String,
}

#[derive(Deserialize)]
pub struct Usage {
    pub prompt_tokens: u64,
    pub completion_tokens: u64,
    pub total_tokens: u64,
}

pub fn system(content: String) -> Message {
    Message {
        role: "system".to_string(),
        content,
    }
}

pub fn user(content: String) -> Message {
    Message {
        role: "user".to_string(),
        content,
    }
}

// Github models
#[derive(Deserialize)]
pub struct PullRequest {
    pub number: u64,
    pub state: String,
    pub draft: bool,
    pub title: String,
    pub body: String,
    pub user: String,
    pub head: String,
    pub base: String,
    pub changed_files: u64,
    pub additions: u64,
    pub deletions: u64,
}
