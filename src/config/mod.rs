mod loader;

use aries_context::AppConfig;
use dialoguer::Input;
use dialoguer::theme::ColorfulTheme;

pub use self::loader::AppConfigLoader;

pub fn setup() -> anyhow::Result<AppConfig> {
    println!("Welcome to Aries! Let's set up your AI model configuration.");
    let theme = ColorfulTheme::default();

    let base_url_input: String =
        Input::with_theme(&theme).with_prompt("base url").allow_empty(false).interact_text()?;
    let base_url = base_url_input.trim().to_owned();

    let api_key_input: String = Input::with_theme(&theme).with_prompt("api key").allow_empty(false).interact_text()?;
    let api_key = api_key_input.trim().to_owned();

    let model_input: String = Input::with_theme(&theme).with_prompt("model").allow_empty(false).interact_text()?;
    let model = model_input.trim().to_owned();

    Ok(AppConfig { api_key, base_url, model })
}
