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
    pub organization: String,
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

    pub fn github_organization(&self) -> Result<&str, String> {
        let organization = self.github.organization.trim();
        if organization.is_empty() || organization.contains('/') {
            return Err("github.organization must be a non-empty organization name".to_string());
        }
        Ok(organization)
    }
}
