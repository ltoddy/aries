#[derive(thiserror::Error, Debug)]
pub enum GrepError {
    #[error("regex error: {0}")]
    Regex(#[from] grep_regex::Error),
    #[error("globset error: {0}")]
    Globset(#[from] globset::Error),
    #[error("search error: {0}")]
    Search(#[from] std::io::Error),
    #[error("internal error: result collector still has outstanding references")]
    CollectorStillShared,
}
