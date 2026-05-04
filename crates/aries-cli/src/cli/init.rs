use aries_config::AriesConfigLoader;
use aries_context::GlobalContext;
use clap::Subcommand;

use crate::theme::Theme;

#[derive(Subcommand, Debug, Clone)]
pub enum InitCommand {
    /// Initialize with OpenAI Compatible provider
    OpenaiCompatible {
        /// Base URL for the API
        #[arg(long)]
        base_url: String,
        /// API key for authentication
        #[arg(long)]
        api_key: String,
        /// Model name to use
        #[arg(long)]
        model: String,
    },
    /// Initialize with Azure provider
    Azure {
        /// Azure endpoint URL
        #[arg(long)]
        azure_endpoint: String,
        /// API key for authentication
        #[arg(long)]
        api_key: String,
        /// Azure API version
        #[arg(long)]
        api_version: String,
        /// Model name to use
        #[arg(long)]
        model: String,
    },
}

pub async fn execute(gctx: GlobalContext, command: InitCommand) -> anyhow::Result<()> {
    let theme = Theme::default();

    let loader = AriesConfigLoader::new(gctx.config_dir);
    let config = command.into_config();
    loader.save(&config).await?;
    println!(
        "{}",
        theme.green_text(&format!("Configuration saved to {}", loader.file_path().display()))
    );
    Ok(())
}

impl InitCommand {
    fn into_config(self) -> aries_config::AriesConfig {
        match self {
            Self::OpenaiCompatible { api_key, base_url, model } => {
                aries_config::AriesConfig::OpenAICompatible(aries_config::OpenAICompatibleConfig {
                    api_key,
                    base_url,
                    model,
                })
            },
            Self::Azure { api_key, azure_endpoint, api_version, model } => {
                aries_config::AriesConfig::Azure(aries_config::AzureConfig {
                    api_key,
                    azure_endpoint,
                    api_version,
                    model,
                })
            },
        }
    }
}
