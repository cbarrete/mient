use serde::Deserialize;
use std::io::prelude::Read;

#[derive(Debug)]
pub struct MientConfigError {
    message: String,
}

impl std::fmt::Display for MientConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for MientConfigError {}

#[derive(Deserialize)]
struct UserConfig {
    user: String,
    homeserver: String,
    device_id: String, // TODO option and make one/write it out if absent
    password_cmd: Vec<String>,
}

#[derive(Deserialize, Debug)]
pub struct MientConfig {
    pub user: String,
    pub homeserver: String,
    pub password: String,
    pub device_id: String, // TODO option and make one/write it out if absent
}

impl MientConfig {
    pub fn get(config_path: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let mut file = std::fs::File::open(config_path)?;
        let mut contents = String::new();
        file.read_to_string(&mut contents)?;
        let user_config = serde_json::from_str::<UserConfig>(&contents)?;
        Self::make(user_config)
    }

    fn make(user_config: UserConfig) -> Result<Self, Box<dyn std::error::Error>> {
        if user_config.password_cmd.len() < 1 {
            return Err(Box::new(MientConfigError { message: String::from("Invalid password command") }));
        }
        let password = std::process::Command::new(&user_config.password_cmd[0])
            .args(&user_config.password_cmd[1..])
            .output()?
            .stdout;
        let password = String::from_utf8(password)?.trim().into();
        let config = MientConfig {
            user: user_config.user,
            homeserver: user_config.homeserver,
            password,
            device_id: user_config.device_id,
        };
        Ok(config)
    }
}
