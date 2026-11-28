mod activity;
mod error;
mod native;
mod position;
mod sprites;
mod view;
#[cfg(target_os = "linux")]
mod x11;

pub use self::activity::{PetActivity, PetSession};
pub use self::error::PetError;
#[cfg(target_os = "macos")]
pub use self::native::hide_main;
pub use self::native::show_main;
#[cfg(target_os = "macos")]
pub use self::view::is_available;
pub use self::view::{init, open_workspace, set_visible};
#[cfg(target_os = "linux")]
pub use self::x11::Mover;
