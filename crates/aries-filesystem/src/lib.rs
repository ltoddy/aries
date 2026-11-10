pub mod document;
pub mod lock;
#[cfg(test)]
mod tests;
pub mod walk;

use std::path::Path;

pub fn path_to_slug(path: impl AsRef<Path>) -> String {
    let path = path.as_ref();
    let path = std::fs::canonicalize(path).unwrap_or_else(|_| path.to_owned());

    path.to_string_lossy()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { '-' })
        .collect::<String>()
}
