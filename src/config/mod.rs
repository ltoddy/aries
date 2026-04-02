mod loader;

use dialoguer::Input;
use dialoguer::theme::ColorfulTheme;
use serde::{Deserialize, Serialize};

pub use self::loader::AppConfigLoader;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppConfig {
    pub api_key: String,
    pub base_url: String,
    pub model_name: String,
}

pub fn setup() -> anyhow::Result<AppConfig> {
    println!("Welcome to Aries! Let's set up your AI model configuration.");
    let theme = ColorfulTheme::default();

    let api_key_input: String = Input::with_theme(&theme).with_prompt("api key").allow_empty(false).interact_text()?;
    let api_key = api_key_input.trim().to_owned();

    let base_url_input: String =
        Input::with_theme(&theme).with_prompt("base url").allow_empty(false).interact_text()?;
    let base_url = base_url_input.trim().to_owned();

    let model_name_input: String =
        Input::with_theme(&theme).with_prompt("model name").allow_empty(false).interact_text()?;
    let model_name = model_name_input.trim().to_owned();

    Ok(AppConfig { api_key, base_url, model_name })
}
