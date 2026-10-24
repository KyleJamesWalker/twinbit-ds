use std::f32::consts::TAU;

use crate::{Color, Frame, rng::Rng};

const GRAVITY: f32 = 900.0;

struct Particle {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    life: f32,
    max_life: f32,
    r: f32,
    color: Color,
}

/// Short-lived sparks in world space, so a burst near the hinge spills onto both screens.
#[derive(Default)]
pub struct Particles(Vec<Particle>);

impl Particles {
    pub fn burst(&mut self, rng: &mut Rng, x: f32, y: f32, colors: &[Color], n: usize) {
        for i in 0..n {
            let angle = rng.range(0.0, TAU);
            let speed = rng.range(150.0, 520.0);
            let life = rng.range(0.4, 0.8);
            self.0.push(Particle {
                x,
                y,
                vx: angle.cos() * speed,
                vy: angle.sin() * speed - 150.0,
                life,
                max_life: life,
                r: rng.range(4.0, 10.0),
                color: colors[i % colors.len()],
            });
        }
    }

    pub fn update(&mut self, dt: f32) {
        for p in &mut self.0 {
            p.vy += GRAVITY * dt;
            p.x += p.vx * dt;
            p.y += p.vy * dt;
            p.life -= dt;
        }
        self.0.retain(|p| p.life > 0.0);
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn draw(&self, frame: &mut Frame) {
        for p in &self.0 {
            let k = p.life / p.max_life;
            let [r, g, b, a] = p.color;
            frame.world_circle(p.x, p.y, p.r * (0.4 + 0.6 * k), [r, g, b, a * k]);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bursts_fade_out() {
        let mut p = Particles::default();
        p.burst(&mut Rng::new(1), 0.0, 0.0, &[[1.0; 4]], 10);
        assert!(!p.is_empty());
        p.update(1.0);
        assert!(p.is_empty());
    }
}
