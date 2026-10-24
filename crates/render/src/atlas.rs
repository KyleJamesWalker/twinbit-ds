//! One RGBA texture holding the sprite sheet and a rasterized font.

use std::collections::HashMap;

use twinbit_game::{Align, Sprite};

const SPRITES_PNG: &[u8] = include_bytes!("../../../assets/sprites.png");
const FONT_TTF: &[u8] = include_bytes!("../../../assets/fonts/AtkinsonHyperlegible-Regular.ttf");
const SPRITE_PX: u32 = 64;
/// Glyphs are rasterized once at this height and scaled when drawn; mipmaps keep small text smooth.
const GLYPH_PX: f32 = 48.0;
const PAD: u32 = 4;
pub const SIZE: u32 = 1024;

#[derive(Clone, Copy)]
pub struct Glyph {
    pub uv: [f32; 4],
    /// Offset from the pen position on the baseline to the bitmap's top-left, in `GLYPH_PX` units.
    pub offset: [f32; 2],
    pub size: [f32; 2],
    pub advance: f32,
}

pub struct Atlas {
    pub rgba: Vec<u8>,
    glyphs: HashMap<char, Glyph>,
    ascent: f32,
}

/// A positioned glyph quad in screen pixels.
pub struct Placed {
    pub rect: [f32; 4],
    pub uv: [f32; 4],
}

impl Atlas {
    pub fn build() -> Result<Self, String> {
        let mut rgba = vec![0u8; (SIZE * SIZE * 4) as usize];
        let (w, h, sheet) = decode_png(SPRITES_PNG)?;
        for y in 0..h {
            let src = &sheet[(y * w * 4) as usize..((y + 1) * w * 4) as usize];
            let dst = (y * SIZE * 4) as usize;
            rgba[dst..dst + src.len()].copy_from_slice(src);
        }

        let font = fontdue::Font::from_bytes(FONT_TTF, fontdue::FontSettings::default())?;
        let ascent = font.horizontal_line_metrics(GLYPH_PX).map_or(GLYPH_PX * 0.8, |m| m.ascent);
        let mut glyphs = HashMap::new();
        let (mut x, mut y, mut row_h) = (PAD, h + PAD, 0);
        for ch in (32u8..127).map(char::from) {
            let (m, bitmap) = font.rasterize(ch, GLYPH_PX);
            let (gw, gh) = (m.width as u32, m.height as u32);
            if x + gw + PAD > SIZE {
                (x, y, row_h) = (PAD, y + row_h + PAD, 0);
            }
            for row in 0..gh {
                for col in 0..gw {
                    let a = bitmap[(row * gw + col) as usize];
                    let i = (((y + row) * SIZE + x + col) * 4) as usize;
                    rgba[i..i + 4].copy_from_slice(&[255, 255, 255, a]);
                }
            }
            let s = SIZE as f32;
            glyphs.insert(ch, Glyph {
                uv: [x as f32 / s, y as f32 / s, (x + gw) as f32 / s, (y + gh) as f32 / s],
                offset: [m.xmin as f32, -(m.ymin as f32 + gh as f32)],
                size: [gw as f32, gh as f32],
                advance: m.advance_width,
            });
            x += gw + PAD;
            row_h = row_h.max(gh);
        }
        Ok(Self { rgba, glyphs, ascent })
    }

    pub fn sprite_uv(&self, sprite: Sprite) -> [f32; 4] {
        let i = Sprite::ALL.iter().position(|&s| s == sprite).unwrap_or(0) as f32;
        let (s, cell) = (SIZE as f32, SPRITE_PX as f32);
        // Half a texel in from each edge so filtering never reads the neighboring cell.
        [(i * cell + 0.5) / s, 0.5 / s, ((i + 1.0) * cell - 0.5) / s, (cell - 0.5) / s]
    }

    /// Lays out one line of text; unknown characters advance like a space.
    pub fn layout(&self, text: &str, x: f32, y: f32, size: f32, align: Align) -> Vec<Placed> {
        let scale = size / GLYPH_PX;
        let space = self.glyphs.get(&' ').map_or(GLYPH_PX * 0.3, |g| g.advance);
        let width: f32 = text.chars().map(|c| self.glyphs.get(&c).map_or(space, |g| g.advance)).sum::<f32>() * scale;
        let mut pen = match align {
            Align::Left => x,
            Align::Center => x - width / 2.0,
            Align::Right => x - width,
        };
        let baseline = y + self.ascent * scale;
        let mut out = Vec::new();
        for c in text.chars() {
            let Some(g) = self.glyphs.get(&c) else {
                pen += space * scale;
                continue;
            };
            if g.size[0] > 0.0 {
                let (gx, gy) = (pen + g.offset[0] * scale, baseline + g.offset[1] * scale);
                out.push(Placed { rect: [gx, gy, g.size[0] * scale, g.size[1] * scale], uv: g.uv });
            }
            pen += g.advance * scale;
        }
        out
    }
}

fn decode_png(bytes: &[u8]) -> Result<(u32, u32, Vec<u8>), String> {
    let decoder = png::Decoder::new(bytes);
    let mut reader = decoder.read_info().map_err(|e| e.to_string())?;
    let mut buf = vec![0; reader.output_buffer_size()];
    let info = reader.next_frame(&mut buf).map_err(|e| e.to_string())?;
    if info.color_type != png::ColorType::Rgba || info.bit_depth != png::BitDepth::Eight {
        return Err(format!("sprite sheet must be 8-bit RGBA, got {:?} {:?}", info.color_type, info.bit_depth));
    }
    buf.truncate(info.buffer_size());
    Ok((info.width, info.height, buf))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_and_lays_out_text() {
        let atlas = Atlas::build().unwrap();
        assert!(atlas.glyphs.contains_key(&'A'));
        let left = atlas.layout("Hi", 0.0, 0.0, 24.0, Align::Left);
        let centered = atlas.layout("Hi", 100.0, 0.0, 24.0, Align::Center);
        assert_eq!(left.len(), 2);
        assert!(centered[0].rect[0] < 100.0 && centered[1].rect[0] > 100.0 - 24.0);
    }
}
