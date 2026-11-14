use aries_init::{GlobalContext, ModelConfig, Provider, SettingLoader};
use clap::Parser;
use dialoguer::theme::ColorfulTheme;
use dialoguer::{Input, Select};

use crate::Result;
use crate::error::Error;

#[derive(Clone, Debug, Parser)]
#[command(about = "Add a new model configuration")]
pub struct AddModelArgs {}

pub async fn execute(_: AddModelArgs, gctx: GlobalContext) -> Result<()> {
    let loader = SettingLoader::new(gctx.root_dir());
    let mut setting = loader.load().await.map_err(Error::load_setting)?;

    println!("Current models:");
    let table = setting.table();
    table.printstd();

    let theme = ColorfulTheme::default();
    let providers: [Provider; 3] = [Provider::Azure, Provider::DeepSeek, Provider::OpenAI];
    let items = providers.iter().map(|p| p.to_string()).collect::<Vec<_>>();
    let provider = &providers[Select::with_theme(&theme)
        .with_prompt("provider")
        .items(&items)
        .default(0)
        .interact()
        .map_err(Error::dialog)?];

    let model = match provider {
        Provider::Azure => {
            let azure_endpoint = Input::<String>::with_theme(&theme)
                .with_prompt("azure endpoint")
                .allow_empty(false)
                .default(String::from("https://{your-resource-name}.openai.azure.com"))
                .interact_text()
                .map_err(Error::dialog)?;

            let api_version = Input::<String>::with_theme(&theme)
                .with_prompt("api version")
                .allow_empty(false)
                .default(String::from("2024-10-21"))
                .interact_text()
                .map_err(Error::dialog)?;

            let api_key = Input::<String>::with_theme(&theme)
                .with_prompt("api key")
                .allow_empty(false)
                .interact_text()
                .map_err(Error::dialog)?;

            let model = Input::<String>::with_theme(&theme)
                .with_prompt("model name")
                .allow_empty(false)
                .interact_text()
                .map_err(Error::dialog)?;

            let alias = Input::<String>::with_theme(&theme)
                .with_prompt("alias")
                .allow_empty(false)
                .interact_text()
                .map_err(Error::dialog)?;

            ModelConfig::azure(alias, model, api_key, azure_endpoint, api_version)
        },
        Provider::DeepSeek => {
            let base_url = Input::<String>::with_theme(&theme)
                .with_prompt("base url")
                .allow_empty(false)
                .default(String::from("https://api.deepseek.com"))
                .interact_text()
                .map_err(Error::dialog)?;

            let api_key = Input::<String>::with_theme(&theme)
                .with_prompt("api key")
                .allow_empty(false)
                .interact_text()
                .map_err(Error::dialog)?;

            let model = Input::<String>::with_theme(&theme)
                .with_prompt("model name")
                .allow_empty(false)
                .interact_text()
                .map_err(Error::dialog)?;

            let alias = Input::<String>::with_theme(&theme)
                .with_prompt("alias")
                .allow_empty(false)
                .interact_text()
                .map_err(Error::dialog)?;

            ModelConfig::deepseek(alias, model, api_key, base_url)
        },
        Provider::OpenAI => {
            let base_url = Input::<String>::with_theme(&theme)
                .with_prompt("base url")
                .allow_empty(false)
                .default(String::from("https://api.openai.com/v1"))
                .interact_text()
                .map_err(Error::dialog)?;

            let api_key = Input::<String>::with_theme(&theme)
                .with_prompt("api key")
                .allow_empty(false)
                .interact_text()
                .map_err(Error::dialog)?;

            let model = Input::<String>::with_theme(&theme)
                .with_prompt("model name")
                .allow_empty(false)
                .interact_text()
                .map_err(Error::dialog)?;

            let alias = Input::<String>::with_theme(&theme)
                .with_prompt("alias")
                .allow_empty(false)
                .interact_text()
                .map_err(Error::dialog)?;

            ModelConfig::openai(alias, model, api_key, base_url)
        },
    };

    setting.add_model(model);
    loader.save(&setting).await.map_err(Error::setting)?;

    println!("model added.");

    Ok(())
}
