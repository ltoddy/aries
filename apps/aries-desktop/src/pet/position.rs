use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use gpui_kit::{App, Bounds, DisplayId, Pixels, Point, Size, Window, point, px, size};
use serde::{Deserialize, Serialize};

use crate::pet::PetError;
use crate::pet::sprites::{CELL_HEIGHT, CELL_WIDTH};

const WINDOW_WIDTH: f32 = 256.;
const WINDOW_HEIGHT: f32 = 256.;
const WINDOW_PADDING_WIDTH: f32 = WINDOW_WIDTH - CELL_WIDTH as f32;
const WINDOW_PADDING_HEIGHT: f32 = WINDOW_HEIGHT - CELL_HEIGHT as f32;
static SAVE_NONCE: AtomicU64 = AtomicU64::new(0);

pub fn window_size(scale: f32) -> Size<Pixels> {
    size(
        px(CELL_WIDTH as f32 * scale + WINDOW_PADDING_WIDTH),
        px(CELL_HEIGHT as f32 * scale + WINDOW_PADDING_HEIGHT),
    )
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
pub struct Position {
    x: f32,
    y: f32,
    display_uuid: String,
}

impl Position {
    pub fn new(origin: Point<Pixels>, display_uuid: String) -> Self {
        Self { x: f32::from(origin.x), y: f32::from(origin.y), display_uuid }
    }

    pub fn from_window(window: &Window, cx: &App) -> Result<Self, PetError> {
        let display = window
            .display(cx)
            .ok_or_else(|| PetError::window("无法确定宠物所在的显示器".into()))?;
        let uuid = display.uuid().map_err(|error| PetError::window(error.to_string()))?;
        let origin = window.window_bounds().get_bounds().origin - display.bounds().origin;
        Ok(Self::new(origin, uuid.to_string()))
    }

    fn path() -> Result<PathBuf, PetError> {
        let home = dirs::home_dir().ok_or_else(PetError::missing_home)?;
        Ok(home.join(".local/share/aries-desktop/pet-position.json"))
    }

    pub fn load() -> Result<Option<Self>, PetError> {
        let path = Self::path()?;
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
            Err(source) => return Err(PetError::io(&path, source)),
        };
        let position: Self =
            serde_json::from_slice(&bytes).map_err(|source| PetError::metadata(&path, source))?;
        if !position.x.is_finite() || !position.y.is_finite() || position.display_uuid.is_empty() {
            return Err(PetError::unsupported("宠物保存的位置无效".into()));
        }
        Ok(Some(Self::new(point(px(position.x), px(position.y)), position.display_uuid)))
    }

    pub fn save(&self) -> Result<(), PetError> {
        let path = Self::path()?;
        let bytes = serde_json::to_vec(self).map_err(|source| PetError::metadata(&path, source))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).map_err(|source| PetError::io(parent, source))?;
        }
        let nonce = SAVE_NONCE.fetch_add(1, Ordering::Relaxed);
        let temporary = path.with_extension(format!("json.{}.{nonce}.tmp", std::process::id()));
        fs::write(&temporary, bytes).map_err(|source| PetError::io(&temporary, source))?;
        fs::rename(&temporary, &path).map_err(|source| PetError::io(&path, source))?;
        Ok(())
    }

    pub fn bounds(position: Option<&Self>, scale: f32, cx: &App) -> Result<Placement, PetError> {
        let desktop = Self::desktop(position, cx)?;
        let window = window_size(scale);
        let visible = desktop.visible();
        if visible.size.width < window.width || visible.size.height < window.height {
            return Err(PetError::unsupported("显示器无法容纳宠物窗口".into()));
        }
        let origin = position.map_or_else(
            || point(visible.right() - window.width, visible.bottom() - window.height),
            |position| point(px(position.x), px(position.y)),
        );
        let origin = point(
            origin.x.clamp(visible.left(), visible.right() - window.width),
            origin.y.clamp(visible.top(), visible.bottom() - window.height),
        );
        Ok(Placement::new(
            desktop.display_id(),
            Bounds::new(origin + desktop.bounds().origin, window),
            Self::new(origin, desktop.uuid().to_string()),
        ))
    }

    pub fn desktop(position: Option<&Self>, cx: &App) -> Result<Desktop, PetError> {
        let displays = cx.displays();
        let selected = position.and_then(|position| {
            displays.iter().find(|display| {
                display.uuid().is_ok_and(|uuid| uuid.to_string() == position.display_uuid)
            })
        });
        let display = selected
            .or_else(|| displays.first())
            .cloned()
            .ok_or_else(|| PetError::window("未找到可用显示器".into()))?;
        let uuid = display.uuid().map_err(|error| PetError::window(error.to_string()))?;
        let bounds = display.bounds();
        let mut visible = display.visible_bounds();
        visible.origin -= bounds.origin;
        Ok(Desktop::new(display.id(), bounds, visible, uuid.to_string()))
    }

    pub fn x(&self) -> f32 {
        self.x
    }

    pub fn y(&self) -> f32 {
        self.y
    }

    pub fn display_uuid(&self) -> &str {
        &self.display_uuid
    }

    #[cfg(target_os = "linux")]
    pub fn moved(&self, delta: Point<Pixels>) -> Self {
        Self::new(
            point(px(self.x + f32::from(delta.x)), px(self.y + f32::from(delta.y))),
            self.display_uuid.clone(),
        )
    }

    pub fn cell(scale: f32) -> Size<Pixels> {
        size(px(CELL_WIDTH as f32 * scale), px(CELL_HEIGHT as f32 * scale))
    }
}

pub struct Placement {
    display_id: DisplayId,
    bounds: Bounds<Pixels>,
    position: Position,
}

impl Placement {
    pub fn new(display_id: DisplayId, bounds: Bounds<Pixels>, position: Position) -> Self {
        Self { display_id, bounds, position }
    }

    pub fn display_id(&self) -> DisplayId {
        self.display_id
    }

    pub fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }

    pub fn position(&self) -> &Position {
        &self.position
    }
}

pub struct Desktop {
    display_id: DisplayId,
    bounds: Bounds<Pixels>,
    visible: Bounds<Pixels>,
    uuid: String,
}

impl Desktop {
    pub fn new(
        display_id: DisplayId,
        bounds: Bounds<Pixels>,
        visible: Bounds<Pixels>,
        uuid: String,
    ) -> Self {
        Self { display_id, bounds, visible, uuid }
    }

    pub fn display_id(&self) -> DisplayId {
        self.display_id
    }

    pub fn bounds(&self) -> Bounds<Pixels> {
        self.bounds
    }

    pub fn visible(&self) -> Bounds<Pixels> {
        self.visible
    }

    pub fn uuid(&self) -> &str {
        &self.uuid
    }
}
