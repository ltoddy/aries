pub mod palette;
mod preferences;
mod state;
mod ui;
pub use self::preferences::{Appearance, Preferences, SendKey};
pub use self::state::SettingsState;
pub use self::ui::SettingsButton;
