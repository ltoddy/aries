use gpui_kit::component::ResizableState;
use gpui_kit::prelude::*;
use gpui_kit::*;

use crate::terminal::TerminalView;
use crate::workspace::workspace_view::WorkspaceView;

pub enum TerminalPlacement {
    Hidden,
    Split,
    Zoomed,
}

impl TerminalPlacement {
    pub fn hidden() -> Self {
        Self::Hidden
    }

    pub fn split() -> Self {
        Self::Split
    }

    pub fn zoomed() -> Self {
        Self::Zoomed
    }

    pub fn is_visible(&self) -> bool {
        !matches!(self, Self::Hidden)
    }

    pub fn is_zoomed(&self) -> bool {
        matches!(self, Self::Zoomed)
    }

    pub fn toggled(&self) -> Self {
        match self {
            Self::Hidden => Self::split(),
            Self::Split | Self::Zoomed => Self::hidden(),
        }
    }

    pub fn zoom_toggled(&self) -> Self {
        match self {
            Self::Hidden => Self::hidden(),
            Self::Split => Self::zoomed(),
            Self::Zoomed => Self::split(),
        }
    }
}

pub struct TerminalState {
    pub view: Option<Entity<TerminalView>>,
    pub placement: TerminalPlacement,
    pub layout: Entity<ResizableState>,
    pub previous_focus: Option<FocusHandle>,
}

impl TerminalState {
    pub fn new(cx: &mut Context<WorkspaceView>) -> Self {
        Self {
            view: None,
            placement: TerminalPlacement::hidden(),
            layout: cx.new(|_| ResizableState::default()),
            previous_focus: None,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.placement.is_visible()
    }

    pub fn is_zoomed(&self) -> bool {
        self.placement.is_zoomed()
    }

    pub fn toggle(&mut self) -> bool {
        self.placement = self.placement.toggled();
        self.placement.is_visible()
    }

    pub fn toggle_zoom(&mut self) {
        self.placement = self.placement.zoom_toggled();
    }
}

impl WorkspaceView {
    pub fn toggle_terminal(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let visible = self.terminal.toggle();
        if visible {
            self.terminal.previous_focus = window.focused(cx);
            if self.terminal.view.is_none() {
                let directory = self.current_directory(cx);
                self.terminal.view = Some(cx.new(|cx| TerminalView::new(directory, cx)));
            }
            if let Some(terminal) = self.terminal.view.as_ref() {
                window.focus(&terminal.read(cx).focus_handle(cx), cx);
            }
        } else {
            let focus =
                self.terminal.previous_focus.take().unwrap_or_else(|| self.focus_handle.clone());
            window.focus(&focus, cx);
        }
        if let Some(terminal) = self.terminal.view.as_ref() {
            terminal.update(cx, |terminal, cx| terminal.set_visible(visible, cx));
        }
        cx.notify();
    }

    pub fn toggle_terminal_zoom(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.terminal.toggle_zoom();
        if let Some(terminal) = self.terminal.view.as_ref() {
            window.focus(&terminal.read(cx).focus_handle(cx), cx);
        }
        cx.notify();
    }
}
