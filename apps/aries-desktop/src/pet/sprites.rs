use std::sync::Arc;
use std::time::Duration;

use gpui_kit::RenderImage;
use image::{Frame, ImageFormat};

use crate::pet::PetError;

pub const CELL_WIDTH: u32 = 192;
pub const CELL_HEIGHT: u32 = 208;

const SHEET: &[u8] = include_bytes!("../../assets/pets/spritesheet.webp");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Clip {
    row: usize,
    durations: &'static [u64],
}

impl Clip {
    pub fn idle() -> Self {
        Self { row: 0, durations: &[280, 110, 110, 140, 140, 320] }
    }

    pub fn waving() -> Self {
        Self { row: 3, durations: &[140, 140, 140, 280] }
    }

    pub fn jumping() -> Self {
        Self { row: 4, durations: &[140, 140, 140, 140, 280] }
    }

    pub fn failed() -> Self {
        Self { row: 5, durations: &[140, 140, 140, 140, 140, 140, 140, 240] }
    }

    pub fn waiting() -> Self {
        Self { row: 6, durations: &[150, 150, 150, 150, 150, 260] }
    }

    pub fn running() -> Self {
        Self { row: 7, durations: &[120, 120, 120, 120, 120, 220] }
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
}

pub struct Sprites {
    rows: Vec<Vec<Arc<RenderImage>>>,
}

impl Sprites {
    pub fn load() -> Result<Self, PetError> {
        let sheet = image::load_from_memory_with_format(SHEET, ImageFormat::WebP)
            .map_err(PetError::image)?
            .into_rgba8();
        if sheet.dimensions() != (CELL_WIDTH * 8, CELL_HEIGHT * 9) {
            return Err(PetError::unsupported(format!(
                "图片尺寸必须为 1536×1872，实际为 {}×{}",
                sheet.width(),
                sheet.height()
            )));
        }
        let mut rows = vec![Vec::new(); 9];
        for clip in [
            Clip::idle(),
            Clip::waving(),
            Clip::jumping(),
            Clip::failed(),
            Clip::waiting(),
            Clip::running(),
        ] {
            for column in 0..clip.frame_count() {
                let mut cell = image::imageops::crop_imm(
                    &sheet,
                    column as u32 * CELL_WIDTH,
                    clip.row as u32 * CELL_HEIGHT,
                    CELL_WIDTH,
                    CELL_HEIGHT,
                )
                .to_image();
                for pixel in cell.pixels_mut() {
                    pixel.0.swap(0, 2);
                }
                rows[clip.row].push(Arc::new(RenderImage::new([Frame::new(cell)])));
            }
        }
        Ok(Self { rows })
    }

    pub fn frame(&self, clip: Clip, frame: usize) -> Arc<RenderImage> {
        self.rows[clip.row][frame].clone()
    }

    pub fn frames(&self) -> impl Iterator<Item = &Arc<RenderImage>> {
        self.rows.iter().flatten()
    }
}
