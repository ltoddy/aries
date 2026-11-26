use gpui_kit::{Pixels, Point, Window};
use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use xcb::{XidNew as _, x};

use crate::pet::PetError;

pub struct Mover {
    connection: xcb::Connection,
    window: xcb::x::Window,
}

impl Mover {
    pub fn new(window: &Window) -> Option<Self> {
        let handle = HasWindowHandle::window_handle(window).ok()?;
        let RawWindowHandle::Xcb(handle) = handle.as_raw() else {
            return None;
        };
        let (connection, _) = xcb::Connection::connect(None).ok()?;
        Some(Self { connection, window: x::Window::new(handle.window.get()) })
    }

    pub fn move_to(&self, origin: Point<Pixels>, scale_factor: f32) -> Result<(), PetError> {
        let x = (f32::from(origin.x) * scale_factor).round() as i32;
        let y = (f32::from(origin.y) * scale_factor).round() as i32;
        self.connection.send_request(&x::ConfigureWindow {
            window: self.window,
            value_list: &[x::ConfigWindow::X(x), x::ConfigWindow::Y(y)],
        });
        self.connection.flush().map_err(|error| PetError::native(error.to_string()))
    }
}
