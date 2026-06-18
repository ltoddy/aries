use aries_context::GlobalContext;
use aries_init::{ModelConfig, Provider, Setting, SettingLoader};
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, Select};

use crate::theme::Theme;

pub async fn execute(gctx: GlobalContext) -> anyhow::Result<()> {
    let theme = Theme::default();

    let mut db = aries_session::connect(&gctx.root_dir).await?;
    let _ = aries_session::migrate(&mut db).await;

    let setting = setup()?;
    let loader = SettingLoader::new(gctx.root_dir);
    loader.save(&setting).await?;

    let file_path = loader.file_path();
    let file_path = file_path.as_ref();

    println!(
        "{}",
        theme.green_text(&format!("Configuration saved successfully to {}", file_path.display()))
    );
    Ok(())
}

fn setup() -> anyhow::Result<Setting> {
    println!("Welcome to Aries! Let's set up your AI model configuration.");
    let theme = ColorfulTheme::default();

    let providers: [Provider; 3] = [Provider::Azure, Provider::DeepSeek, Provider::OpenAI];
    let items = providers.iter().map(|p| p.to_string()).collect::<Vec<_>>();
    let provider = &providers
        [Select::with_theme(&theme).with_prompt("provider").items(&items).default(0).interact()?];

    let model = match provider {
        Provider::Anthropic => {
            let base_url = Input::<String>::with_theme(&theme)
                .with_prompt("base url")
                .allow_empty(false)
                .default(String::from("https://api.openai.com/v1"))
                .interact_text()?;

            let api_key = Input::<String>::with_theme(&theme)
                .with_prompt("api key")
                .allow_empty(false)
                .interact_text()?;

            let model = Input::<String>::with_theme(&theme)
                .with_prompt("model name")
                .allow_empty(false)
                .interact_text()?;

            let max_tokens = Input::<u64>::with_theme(&theme)
                .with_prompt("max tokens")
                .default(2000)
                .allow_empty(false)
                .interact_text()?;

            let alias = Input::<String>::with_theme(&theme)
                .with_prompt("alias")
                .allow_empty(false)
                .interact_text()?;

            ModelConfig::anthropic(alias, model, api_key, base_url, max_tokens)
        },
        Provider::Azure => {
            let azure_endpoint = Input::<String>::with_theme(&theme)
                .with_prompt("azure endpoint")
                .allow_empty(false)
                .default(String::from("https://{your-resource-name}.openai.azure.com"))
                .interact_text()?;

            let api_version = Input::<String>::with_theme(&theme)
                .with_prompt("api version")
                .allow_empty(false)
                .default(String::from("2024-10-21"))
                .interact_text()?;

            let api_key = Input::<String>::with_theme(&theme)
                .with_prompt("api key")
                .allow_empty(false)
                .interact_text()?;

            let model = Input::<String>::with_theme(&theme)
                .with_prompt("model name")
                .allow_empty(false)
                .interact_text()?;

            let alias = Input::<String>::with_theme(&theme)
                .with_prompt("alias")
                .allow_empty(false)
                .interact_text()?;

            ModelConfig::azure(alias, model, api_key, azure_endpoint, api_version)
        },
        Provider::DeepSeek => {
            let base_url = Input::<String>::with_theme(&theme)
                .with_prompt("base url")
                .allow_empty(false)
                .default(String::from("https://api.deepseek.com"))
                .interact_text()?;

            let api_key = Input::<String>::with_theme(&theme)
                .with_prompt("api key")
                .allow_empty(false)
                .interact_text()?;

            let model = Input::<String>::with_theme(&theme)
                .with_prompt("model name")
                .allow_empty(false)
                .interact_text()?;

            let alias = Input::<String>::with_theme(&theme)
                .with_prompt("alias")
                .allow_empty(false)
                .interact_text()?;

            ModelConfig::deepseek(alias, model, api_key, base_url)
        },
        Provider::OpenAI => {
            let base_url = Input::<String>::with_theme(&theme)
                .with_prompt("base url")
                .allow_empty(false)
                .default(String::from("https://api.openai.com/v1"))
                .interact_text()?;

            let api_key = Input::<String>::with_theme(&theme)
                .with_prompt("api key")
                .allow_empty(false)
                .interact_text()?;

            let model = Input::<String>::with_theme(&theme)
                .with_prompt("model name")
                .allow_empty(false)
                .interact_text()?;

            let alias = Input::<String>::with_theme(&theme)
                .with_prompt("alias")
                .allow_empty(false)
                .interact_text()?;

            ModelConfig::openai(alias, model, api_key, base_url)
        },
    };

    let setting = Setting::new(model);

    Ok(setting)
}
