use gpui_kit::*;

pub struct LayoutSizes {
    font_size: Pixels,
    viewport: Size<Pixels>,
}

impl LayoutSizes {
    pub fn new(font_size: Pixels, viewport: Size<Pixels>) -> Self {
        Self { font_size, viewport }
    }

    pub fn content_minimum(&self) -> Pixels {
        self.font_size * 26.
    }

    pub fn sidebar_minimum(&self) -> Pixels {
        self.font_size * 13.75
    }

    pub fn sidebar_maximum(&self) -> Pixels {
        (self.viewport.width - self.content_minimum() - self.font_size).min(self.font_size * 30.)
    }

    pub fn sidebar_available(&self) -> Option<Pixels> {
        let maximum = self.sidebar_maximum();
        (maximum >= self.sidebar_minimum()).then_some(maximum)
    }

    pub fn sidebar_default(&self) -> Pixels {
        self.font_size * 16.25
    }

    pub fn sidebar_width(&self, stored: Option<Pixels>) -> Pixels {
        stored
            .unwrap_or_else(|| self.sidebar_default())
            .clamp(self.sidebar_minimum(), self.sidebar_maximum())
    }

    pub fn sidebar_focus_window(&self) -> Pixels {
        self.font_size * 40.75
    }

    pub fn title_inset(&self) -> Pixels {
        if cfg!(target_os = "macos") { px(80.) } else { px(12.) }
    }

    pub fn terminal(&self) -> TerminalSizes {
        TerminalSizes::new(self.font_size, self.viewport.height)
    }
}

#[derive(Clone, Copy)]
pub struct SidebarLayout {
    visible: bool,
    width: Pixels,
    minimum: Pixels,
    maximum: Pixels,
    available: Option<Pixels>,
}

impl SidebarLayout {
    pub fn new(visible: bool, stored: Option<Pixels>, sizes: &LayoutSizes) -> Self {
        let available = sizes.sidebar_available();
        let visible = visible && available.is_some();
        let width = if visible { sizes.sidebar_width(stored) } else { Pixels::ZERO };
        Self {
            visible,
            width,
            minimum: sizes.sidebar_minimum(),
            maximum: sizes.sidebar_maximum(),
            available,
        }
    }

    pub fn is_visible(&self) -> bool {
        self.visible
    }

    pub fn minimum(&self) -> Pixels {
        self.minimum
    }

    pub fn maximum(&self) -> Pixels {
        self.maximum
    }

    pub fn available(&self) -> Option<Pixels> {
        self.available
    }

    pub fn title_width(&self, sizes: &LayoutSizes) -> Pixels {
        self.width - sizes.title_inset()
    }
}

pub struct TerminalSizes {
    split: Pixels,
    content_height: Pixels,
    content_minimum: Pixels,
    minimum: Pixels,
    maximum: Pixels,
}

impl TerminalSizes {
    pub fn new(font_size: Pixels, viewport_height: Pixels) -> Self {
        let height = (viewport_height - font_size * 2.625).max(font_size);
        let content_minimum = (font_size * 12.).min(height * 0.6);
        let minimum = (font_size * 8.).min(height * 0.4);
        let maximum = height - content_minimum;
        let split = (height / 3.).clamp(minimum, maximum);
        Self { split, content_height: height - split, content_minimum, minimum, maximum }
    }

    pub fn split(&self) -> Pixels {
        self.split
    }

    pub fn content_height(&self) -> Pixels {
        self.content_height
    }

    pub fn content_minimum(&self) -> Pixels {
        self.content_minimum
    }

    pub fn minimum(&self) -> Pixels {
        self.minimum
    }

    pub fn maximum(&self) -> Pixels {
        self.maximum
    }
}
