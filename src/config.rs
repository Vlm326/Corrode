use serde::Deserialize;

#[derive(Deserialize, Debug, Clone)]
pub struct Config {
    pub app: AppConfig,
    pub github: GithubConfig,
    pub openai: OpenAiConfig,
    pub db: DbConfig,
}

#[derive(Deserialize, Debug, Clone)]
pub struct AppConfig {
    pub poll_interval_secs: u64,
}

#[derive(Deserialize, Debug, Clone)]
pub struct GithubConfig {
    pub token: String,
    pub repo: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct OpenAiConfig {
    pub api_key: String,
    pub model: String,
    #[serde(default = "default_openai_url")]
    pub base_url: String,
}

#[derive(Deserialize, Debug, Clone)]
pub struct DbConfig {
    pub url: String,
}

fn default_openai_url() -> String {
    "https://api.openai.com/v1".to_string()
}

impl Config {
    pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
        let config_str = std::fs::read_to_string("Config.toml")?;
        Ok(toml::from_str(&config_str)?)
    }

    pub fn github_repo_parts(&self) -> Result<(&str, &str), String> {
        self.github
            .repo
            .split_once('/')
            .ok_or_else(|| "github.repo must have the form owner/repository".to_string())
    }
}
