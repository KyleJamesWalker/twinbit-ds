use crate::scene3d::Scene3d;
use crate::{BOTTOM_OFFSET, SCREEN_H, SCREEN_W, font};

pub type Color = [f32; 4];

/// Cells of the sprite sheet, left to right.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sprite {
    Star,
    Rock,
    Gem,
    Heart,
}

impl Sprite {
    pub const ALL: [Sprite; 4] = [Sprite::Star, Sprite::Rock, Sprite::Gem, Sprite::Heart];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Align {
    Left,
    Center,
    Right,
}

#[derive(Clone, Debug, PartialEq)]
pub enum Shape {
    Rect { x: f32, y: f32, w: f32, h: f32, color: Color },
    Circle { x: f32, y: f32, r: f32, color: Color },
    /// Centered on (x, y), `size` pixels across, rotated by `angle` radians.
    Sprite { sprite: Sprite, x: f32, y: f32, size: f32, angle: f32, color: Color },
    /// TrueType text; `y` is the top of the line and `size` its height in pixels.
    Text { text: String, x: f32, y: f32, size: f32, color: Color, align: Align },
}

#[derive(Clone, Debug, Default)]
pub struct Canvas {
    pub clear: Color,
    /// Drawn first, under the 2D shapes.
    pub scene: Option<Scene3d>,
    pub shapes: Vec<Shape>,
}

impl Canvas {
    pub fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, color: Color) {
        self.shapes.push(Shape::Rect { x, y, w, h, color });
    }

    pub fn circle(&mut self, x: f32, y: f32, r: f32, color: Color) {
        self.shapes.push(Shape::Circle { x, y, r, color });
    }

    pub fn sprite(&mut self, sprite: Sprite, x: f32, y: f32, size: f32, angle: f32, color: Color) {
        self.shapes.push(Shape::Sprite { sprite, x, y, size, angle, color });
    }

    /// TrueType text. `text` and `text_centered` draw the 3x5 bitmap font instead.
    pub fn label(&mut self, text: impl Into<String>, x: f32, y: f32, size: f32, color: Color, align: Align) {
        self.shapes.push(Shape::Text { text: text.into(), x, y, size, color, align });
    }

    pub fn text(&mut self, x: f32, y: f32, scale: f32, color: Color, s: &str) {
        let mut cx = x;
        for ch in s.chars() {
            let glyph = font::glyph(ch);
            for row in 0..5 {
                for col in 0..3 {
                    if font::lit(glyph, row, col) {
                        let (px, py) = (cx + col as f32 * scale, y + row as f32 * scale);
                        self.rect(px, py, scale, scale, color);
                    }
                }
            }
            cx += font::ADVANCE * scale;
        }
    }

    pub fn text_centered(&mut self, y: f32, scale: f32, color: Color, s: &str) {
        let w = font::width(s) * scale;
        self.text((SCREEN_W - w) / 2.0, y, scale, color, s);
    }
}

#[derive(Clone, Debug, Default)]
pub struct Frame {
    pub top: Canvas,
    pub bottom: Canvas,
}

impl Frame {
    /// Draws a circle in world space, which runs from the top screen through the hinge to the bottom one.
    pub fn world_sprite(&mut self, sprite: Sprite, x: f32, wy: f32, size: f32, angle: f32, color: Color) {
        let r = size / 2.0;
        if wy - r < SCREEN_H {
            self.top.sprite(sprite, x, wy, size, angle, color);
        }
        if wy + r > BOTTOM_OFFSET {
            self.bottom.sprite(sprite, x, wy - BOTTOM_OFFSET, size, angle, color);
        }
    }

    pub fn world_circle(&mut self, x: f32, wy: f32, r: f32, color: Color) {
        if wy - r < SCREEN_H {
            self.top.circle(x, wy, r, color);
        }
        if wy + r > BOTTOM_OFFSET {
            self.bottom.circle(x, wy - BOTTOM_OFFSET, r, color);
        }
    }
}
