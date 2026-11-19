#[cfg(target_os = "macos")]
use std::cell::Cell;

use gpui_kit::{App, Window};
#[cfg(target_os = "macos")]
use objc2::{
    DefinedClass, MainThreadMarker, MainThreadOnly, class, define_class, msg_send, rc::Retained,
    runtime::NSObject, sel,
};
#[cfg(target_os = "macos")]
use objc2_app_kit::{
    NSApplication, NSControlStateValueOff, NSControlStateValueOn, NSMenu, NSMenuItem, NSView,
    NSWindow, NSWindowStyleMask, NSWorkspace,
};
#[cfg(target_os = "macos")]
use raw_window_handle::{HasWindowHandle, RawWindowHandle};

#[cfg(target_os = "macos")]
define_class!(
    #[unsafe(super = NSObject)]
    #[thread_kind = MainThreadOnly]
    #[name = "AriesPetMenuTarget"]
    #[ivars = Cell<Option<usize>>]
    struct MenuTarget;

    impl MenuTarget {
        #[unsafe(method(selectPetMenuItem:))]
        fn select(&self, item: &NSMenuItem) {
            self.ivars().set(usize::try_from(item.tag()).ok().filter(|ix| *ix < 3));
        }
    }
);

#[cfg(target_os = "macos")]
impl MenuTarget {
    fn new(mtm: MainThreadMarker) -> Retained<Self> {
        let this = Self::alloc(mtm).set_ivars(Cell::new(None));
        unsafe { msg_send![super(this), init] }
    }
}

pub fn show(window: &mut Window, cx: &mut App) {
    #[cfg(target_os = "macos")]
    if let Some(native) = native_window(window) {
        cx.foreground_executor()
            .spawn(async move {
                native.setStyleMask(
                    NSWindowStyleMask::Borderless | NSWindowStyleMask::NonactivatingPanel,
                );
                native.setHidesOnDeactivate(false);
                native.setHasShadow(false);
                native.orderFrontRegardless();
            })
            .detach();
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = cx;
        window.activate_window();
    }
}

pub fn restore(window: &mut Window, cx: &mut App) {
    #[cfg(target_os = "macos")]
    if let Some(native) = native_window(window) {
        cx.foreground_executor()
            .spawn(async move {
                if native.isMiniaturized() {
                    native.deminiaturize(None);
                }
            })
            .detach();
    }
    #[cfg(not(target_os = "macos"))]
    let _ = (window, cx);
}

pub fn menu(
    window: &Window,
    paused: bool,
    callback: impl FnOnce(Option<usize>, &mut App) + 'static,
    cx: &App,
) {
    #[cfg(target_os = "macos")]
    let native = native_view(window).and_then(|view| {
        let mtm = MainThreadMarker::new()?;
        let event = NSApplication::sharedApplication(mtm).currentEvent()?;
        Some((view, event, mtm))
    });
    #[cfg(not(target_os = "macos"))]
    let _ = (window, paused);
    let async_cx = cx.to_async();
    cx.foreground_executor()
        .spawn(async move {
            #[cfg(target_os = "macos")]
            let selected = native.and_then(|(view, event, mtm)| {
                view.window()?;
                let target = MenuTarget::new(mtm);
                let menu = NSMenu::new(mtm);
                menu.setAutoenablesItems(false);
                for (tag, title) in [(0, c"打开 Aries…"), (1, c"暂停动画"), (2, c"隐藏桌宠")]
                {
                    if tag == 2 {
                        menu.addItem(&NSMenuItem::separatorItem(mtm));
                    }
                    let item = NSMenuItem::new(mtm);
                    let title: Retained<_> = unsafe {
                        msg_send![class!(NSString), stringWithUTF8String: title.as_ptr()]
                    };
                    item.setTitle(&*title);
                    item.setTag(tag);
                    item.setState(if tag == 1 && paused {
                        NSControlStateValueOn
                    } else {
                        NSControlStateValueOff
                    });
                    unsafe {
                        item.setAction(Some(sel!(selectPetMenuItem:)));
                        item.setTarget(Some(&target));
                    }
                    menu.addItem(&item);
                }
                NSMenu::popUpContextMenu_withEvent_forView(&menu, &event, &view);
                target.ivars().get()
            });
            #[cfg(not(target_os = "macos"))]
            let selected = None;
            async_cx.update(|cx| callback(selected, cx));
        })
        .detach();
}

pub fn reduced_motion() -> bool {
    #[cfg(target_os = "macos")]
    {
        MainThreadMarker::new().is_some()
            && NSWorkspace::sharedWorkspace().accessibilityDisplayShouldReduceMotion()
    }
    #[cfg(not(target_os = "macos"))]
    false
}

#[cfg(target_os = "macos")]
fn native_window(window: &Window) -> Option<Retained<NSWindow>> {
    native_view(window)?.window()
}

#[cfg(target_os = "macos")]
fn native_view(window: &Window) -> Option<Retained<NSView>> {
    MainThreadMarker::new()?;
    let handle = HasWindowHandle::window_handle(window).ok()?;
    let RawWindowHandle::AppKit(handle) = handle.as_raw() else {
        return None;
    };
    unsafe { Retained::retain(handle.ns_view.cast::<NSView>().as_ptr()) }
}
