use gpui_kit::Window;

use crate::pet::PetError;

#[cfg(target_os = "macos")]
mod macos {
    use gpui_kit::Window;
    use objc2::MainThreadMarker;
    use objc2::rc::Retained;
    use objc2::runtime::AnyObject;
    use objc2_app_kit::{NSPanel, NSView, NSWindow};
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    use crate::pet::PetError;

    fn native_window(window: &Window) -> Result<Retained<NSWindow>, PetError> {
        let _main_thread = MainThreadMarker::new()
            .ok_or_else(|| PetError::native("AppKit 窗口只能在主线程操作".into()))?;
        let handle = HasWindowHandle::window_handle(window).map_err(PetError::native_handle)?;
        let RawWindowHandle::AppKit(appkit) = handle.as_raw() else {
            return Err(PetError::native("GPUI 窗口未提供 AppKit 句柄".into()));
        };
        let object = unsafe { appkit.ns_view.cast::<AnyObject>().as_ptr().as_ref() }
            .ok_or_else(|| PetError::native("GPUI AppKit NSView 句柄为空".into()))?;
        let view = object
            .downcast_ref::<NSView>()
            .ok_or_else(|| PetError::native("GPUI AppKit 句柄不是 NSView".into()))?;
        view.window().ok_or_else(|| PetError::native("GPUI NSView 尚未绑定窗口".into()))
    }

    pub fn configure(window: &mut Window) -> Result<(), PetError> {
        let native = native_window(window)?;
        let object: &AnyObject = &native;
        let panel = object
            .downcast_ref::<NSPanel>()
            .ok_or_else(|| PetError::native("宠物 PopUp 窗口不是 NSPanel".into()))?;
        native.setHidesOnDeactivate(false);
        native.setHasShadow(false);
        panel.setBecomesKeyOnlyIfNeeded(true);
        Ok(())
    }

    pub fn hide_main(window: &mut Window) -> Result<(), PetError> {
        native_window(window)?.orderOut(None);
        Ok(())
    }

    pub fn show_main(window: &mut Window) -> Result<(), PetError> {
        let native = native_window(window)?;
        if native.isMiniaturized() {
            native.deminiaturize(None);
        }
        native.orderFront(None);
        window.activate_window();
        Ok(())
    }
}

pub fn configure(window: &mut Window) -> Result<(), PetError> {
    #[cfg(target_os = "macos")]
    {
        self::macos::configure(window)
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = window;
        Ok(())
    }
}

pub fn hide_main(window: &mut Window) -> Result<(), PetError> {
    #[cfg(target_os = "macos")]
    {
        self::macos::hide_main(window)
    }
    #[cfg(not(target_os = "macos"))]
    {
        window.minimize_window();
        Ok(())
    }
}

pub fn show_main(window: &mut Window) -> Result<(), PetError> {
    #[cfg(target_os = "macos")]
    {
        self::macos::show_main(window)
    }
    #[cfg(not(target_os = "macos"))]
    {
        window.activate_window();
        Ok(())
    }
}
