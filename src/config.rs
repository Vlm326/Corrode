use serde::Deserialize;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub poll_interval_secs: u64,
    pub github_token: String,
    pub github_repo: String,
    pub db_url: String,
}

impl Config {
    pub fn load() -> Result<Config, Box<dyn std::error::Error>> {
        let config_str = std::fs::read_to_string("Config.toml")?;
        let config: toml::Value = toml::from_str(&config_str)?;
        Ok(Config {
            poll_interval_secs: config["app"]["poll_interval_secs"].as_integer().unwrap() as u64,
            github_token: config["github"]["token"].as_str().unwrap().to_string(),
            github_repo: config["github"]["repo"].as_str().unwrap().to_string(),
            db_url: config["db"]["url"].as_str().unwrap().to_string(),
        })
    }
}
