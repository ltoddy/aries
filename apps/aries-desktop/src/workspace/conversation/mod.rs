mod attachment;
mod composer;
mod media;
pub mod model;
mod slash_commands;
mod subagent_message;
mod text;
mod thought_message;
pub mod timeline;
mod tool_content;
pub mod tool_message;
mod user_message;
use gpui_kit::{App, ClickEvent, Window};

pub use self::composer::Composer;
pub use self::slash_commands::CommandMenuAction;
pub use self::text::single_line;

pub type ClickHandler = Box<dyn Fn(&ClickEvent, &mut Window, &mut App)>;
