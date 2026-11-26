use std::sync::Arc;
use std::time::Duration;

use gpui_kit::RenderImage;
use image::{Frame, ImageFormat};

use crate::pet::PetError;

pub const CELL_WIDTH: u32 = 192;
pub const CELL_HEIGHT: u32 = 208;

const IDLE: &[u8] = include_bytes!("../../assets/pets/aries/idle.webp");
const WAVING: &[u8] = include_bytes!("../../assets/pets/aries/waving.webp");
const JUMPING: &[u8] = include_bytes!("../../assets/pets/aries/jumping.webp");
const FAILED: &[u8] = include_bytes!("../../assets/pets/aries/failed.webp");
const WAITING: &[u8] = include_bytes!("../../assets/pets/aries/waiting.webp");
const RUNNING: &[u8] = include_bytes!("../../assets/pets/aries/running.webp");
const RUN_RIGHT: &[u8] = include_bytes!("../../assets/pets/aries/run_right.webp");
const RUN_LEFT: &[u8] = include_bytes!("../../assets/pets/aries/run_left.webp");
const SLEEP: &[u8] = include_bytes!("../../assets/pets/aries/sleep.webp");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clip {
    slot: usize,
    name: &'static str,
    sheet: &'static [u8],
    durations: &'static [u64],
}

impl Clip {
    pub fn all() -> [Self; 9] {
        [
            Self::idle(),
            Self::waving(),
            Self::jumping(),
            Self::failed(),
            Self::waiting(),
            Self::running(),
            Self::run_right(),
            Self::run_left(),
            Self::sleep(),
        ]
    }

    pub fn idle() -> Self {
        Self::new(0, "idle", IDLE, &[280, 110, 110, 140, 140, 320])
    }

    pub fn waving() -> Self {
        Self::new(1, "waving", WAVING, &[140, 140, 140, 280])
    }

    pub fn jumping() -> Self {
        Self::new(2, "jumping", JUMPING, &[140, 140, 140, 140, 280])
    }

    pub fn failed() -> Self {
        Self::new(3, "failed", FAILED, &[140, 140, 140, 140, 140, 140, 140, 240])
    }

    pub fn waiting() -> Self {
        Self::new(4, "waiting", WAITING, &[150, 150, 150, 150, 150, 260])
    }

    pub fn running() -> Self {
        Self::new(5, "running", RUNNING, &[120, 120, 120, 120, 120, 220])
    }

    pub fn run_right() -> Self {
        Self::new(6, "run_right", RUN_RIGHT, &[90, 90, 90, 90, 90, 90, 90, 180])
    }

    pub fn run_left() -> Self {
        Self::new(7, "run_left", RUN_LEFT, &[90, 90, 90, 90, 90, 90, 90, 180])
    }

    pub fn sleep() -> Self {
        Self::new(8, "sleep", SLEEP, &[400, 400, 400, 400, 400, 500])
    }

    fn new(
        slot: usize,
        name: &'static str,
        sheet: &'static [u8],
        durations: &'static [u64],
    ) -> Self {
        Self { slot, name, sheet, durations }
    }

    pub fn duration(self, frame: usize) -> Duration {
        Duration::from_millis(self.durations[frame])
    }

    pub fn total_duration(self) -> Duration {
        Duration::from_millis(self.durations.iter().sum())
    }

    pub fn frame_count(self) -> usize {
        self.durations.len()
    }

    fn load(self) -> Result<Vec<Arc<RenderImage>>, PetError> {
        let sheet = image::load_from_memory_with_format(self.sheet, ImageFormat::WebP)
            .map_err(PetError::image)?
            .into_rgba8();
        let width = CELL_WIDTH * self.frame_count() as u32;
        if sheet.dimensions() != (width, CELL_HEIGHT) {
            return Err(PetError::unsupported(format!(
                "{} 必须为 {}×{}，实际为 {}×{}",
                self.name,
                width,
                CELL_HEIGHT,
                sheet.width(),
                sheet.height()
            )));
        }
        let mut frames = Vec::with_capacity(self.frame_count());
        for column in 0..self.frame_count() {
            let mut cell = image::imageops::crop_imm(
                &sheet,
                column as u32 * CELL_WIDTH,
                0,
                CELL_WIDTH,
                CELL_HEIGHT,
            )
            .to_image();
            for pixel in cell.pixels_mut() {
                pixel.0.swap(0, 2);
            }
            frames.push(Arc::new(RenderImage::new([Frame::new(cell)])));
        }
        Ok(frames)
    }
}

pub struct Sprites {
    clips: Vec<Vec<Arc<RenderImage>>>,
}

impl Sprites {
    pub fn load() -> Result<Self, PetError> {
        let mut clips = vec![Vec::new(); Clip::all().len()];
        for clip in Clip::all() {
            clips[clip.slot] = clip.load()?;
        }
        Ok(Self { clips })
    }

    pub fn frame(&self, clip: Clip, frame: usize) -> Arc<RenderImage> {
        self.clips[clip.slot][frame].clone()
    }

    pub fn frames(&self) -> impl Iterator<Item = &Arc<RenderImage>> {
        self.clips.iter().flatten()
    }
}
