use clap::Subcommand;

#[derive(Subcommand, Debug, Clone)]
pub enum SessionCommand {
    List,
    Prune {
        session_ids: Option<Vec<String>>,
        #[arg(long, default_value_t = false)]
        all: bool,
    },
    Resume {
        session_id: String,
    },
}
