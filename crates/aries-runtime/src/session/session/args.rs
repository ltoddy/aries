#[derive(Debug, Clone, Default)]
pub struct SessionArgs {
    pub bare: bool,
}

impl SessionArgs {
    pub fn new(bare: bool) -> Self {
        Self { bare }
    }
}
