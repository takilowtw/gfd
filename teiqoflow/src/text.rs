//! Shared text rendering for the overlay and icons (ab_glyph rasterised into tiny-skia pixmaps).

use ab_glyph::{Font, FontVec, PxScale, ScaleFont, point};
use tiny_skia::Pixmap;

pub struct TextRenderer {
    fonts: Vec<FontVec>,
}

impl TextRenderer {
    /// Loads the bundled Figtree/Onest SemiBold, then system fonts for scripts they don't cover.
    pub fn load() -> Self {
        let dir = std::env::var("WINDIR").unwrap_or_else(|_| "C:\\Windows".into()) + "\\Fonts\\";
        let mut fonts: Vec<FontVec> = crate::style::UI_SEMIBOLD
            .iter()
            .filter_map(|b| FontVec::try_from_vec(b.to_vec()).ok())
            .collect();
        for (file, index) in [
            ("seguisb.ttf", 0),
            ("segoeui.ttf", 0),
            ("seguisym.ttf", 0),
            ("msyh.ttc", 0),
            ("YuGothM.ttc", 0),
            ("malgun.ttf", 0),
            ("Nirmala.ttc", 0),
            ("Nirmala.ttf", 0),
        ] {
            if let Ok(bytes) = std::fs::read(format!("{dir}{file}")) {
                if let Ok(f) = FontVec::try_from_vec_and_index(bytes, index) {
                    fonts.push(f);
                }
            }
        }
        Self { fonts }
    }

    fn font_for(&self, c: char) -> Option<&FontVec> {
        self.fonts.iter().find(|f| f.glyph_id(c).0 != 0).or(self.fonts.first())
    }

    pub fn width(&self, text: &str, size: f32) -> f32 {
        text.chars()
            .filter_map(|c| self.font_for(c).map(|f| f.as_scaled(PxScale::from(size)).h_advance(f.glyph_id(c))))
            .sum()
    }

    /// Shortens `text` with an ellipsis so it fits in `max_width`.
    pub fn elide(&self, text: &str, size: f32, max_width: f32) -> String {
        if self.width(text, size) <= max_width {
            return text.to_string();
        }
        let ell = self.width("…", size);
        let mut out = String::new();
        let mut w = 0.0;
        for c in text.chars() {
            let cw = self.width(&c.to_string(), size);
            if w + cw + ell > max_width {
                break;
            }
            w += cw;
            out.push(c);
        }
        out.trim_end().to_string() + "…"
    }

    /// Draws text with its vertical centre at `cy`. Colour is straight (non-premultiplied) RGBA.
    pub fn draw(&self, pm: &mut Pixmap, text: &str, x: f32, cy: f32, size: f32, rgba: [u8; 4]) {
        let Some(primary) = self.fonts.first() else { return };
        let scale = PxScale::from(size);
        let sp = primary.as_scaled(scale);
        let baseline = cy + (sp.ascent() + sp.descent()) / 2.0;
        let (w, h) = (pm.width() as i32, pm.height() as i32);
        let data = pm.data_mut();
        let mut pen = x;
        for c in text.chars() {
            let Some(font) = self.font_for(c) else { continue };
            let id = font.glyph_id(c);
            let glyph = id.with_scale_and_position(scale, point(pen, baseline));
            pen += font.as_scaled(scale).h_advance(id);
            let Some(outline) = font.outline_glyph(glyph) else { continue };
            let b = outline.px_bounds();
            outline.draw(|gx, gy, cov| {
                let px = b.min.x as i32 + gx as i32;
                let py = b.min.y as i32 + gy as i32;
                if px < 0 || py < 0 || px >= w || py >= h {
                    return;
                }
                let a = cov.clamp(0.0, 1.0) * rgba[3] as f32 / 255.0;
                let i = ((py * w + px) * 4) as usize;
                for ch in 0..3 {
                    let src = rgba[ch] as f32 * a;
                    data[i + ch] = (src + data[i + ch] as f32 * (1.0 - a)).round() as u8;
                }
                data[i + 3] = (255.0 * a + data[i + 3] as f32 * (1.0 - a)).round() as u8;
            });
        }
    }
}
