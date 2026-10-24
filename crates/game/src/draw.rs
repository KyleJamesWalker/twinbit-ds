use crate::scene3d::Scene3d;
use crate::{BOTTOM_OFFSET, SCREEN_H, SCREEN_W, font};

pub type Color = [f32; 4];

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Shape {
    Rect { x: f32, y: f32, w: f32, h: f32, color: Color },
    Circle { x: f32, y: f32, r: f32, color: Color },
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
    pub fn world_circle(&mut self, x: f32, wy: f32, r: f32, color: Color) {
        if wy - r < SCREEN_H {
            self.top.circle(x, wy, r, color);
        }
        if wy + r > BOTTOM_OFFSET {
            self.bottom.circle(x, wy - BOTTOM_OFFSET, r, color);
        }
    }
}
