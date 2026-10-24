use super::{Microgame, Status};
use crate::{BOTTOM_OFFSET, Frame, Input, Sfx, Sprite, WORLD_H, particles::Particles, rng::Rng};

const RADIUS: f32 = 36.0;
const GRAB: f32 = 72.0;
const FALL: f32 = 360.0;
const STAR: [f32; 4] = [1.0, 0.86, 0.2, 1.0];
const SPARKS: usize = 24;

struct Star {
    x: f32,
    y: f32,
    delay: f32,
    caught: bool,
}

/// Stars fall from the top screen through the hinge; tap them on the bottom screen.
pub struct Stars {
    stars: Vec<Star>,
    speed: f32,
    rng: Rng,
    sparks: Particles,
}

impl Stars {
    pub fn new(rng: &mut Rng, speed: f32) -> Self {
        let stars = (0..3)
            .map(|i| Star {
                x: rng.range(80.0, 944.0),
                y: -RADIUS,
                delay: i as f32 * 0.6 / speed,
                caught: false,
            })
            .collect();
        Self { stars, speed, rng: Rng::new(rng.next_u32() as u64), sparks: Particles::default() }
    }
}

impl Microgame for Stars {
    fn prompt(&self) -> &'static str {
        "CATCH!"
    }

    fn duration(&self) -> f32 {
        7.0 / self.speed
    }

    /// The last burst can still be in flight when the timer runs out.
    fn win_on_timeout(&self) -> bool {
        self.stars.iter().all(|s| s.caught)
    }

    fn update(&mut self, input: &Input, dt: f32, sfx: &mut Vec<Sfx>) -> Status {
        self.sparks.update(dt);
        for s in self.stars.iter_mut().filter(|s| !s.caught) {
            if s.delay > 0.0 {
                s.delay -= dt;
            } else {
                s.y += FALL * self.speed * dt;
            }
        }
        if let (true, Some([tx, ty])) = (input.touch_started, input.touch) {
            let wy = ty + BOTTOM_OFFSET;
            if let Some(s) = self
                .stars
                .iter_mut()
                .filter(|s| !s.caught && (s.x - tx).hypot(s.y - wy) < GRAB)
                .min_by(|a, b| a.y.total_cmp(&b.y).reverse())
            {
                s.caught = true;
                sfx.push(Sfx::Catch);
                self.sparks.burst(&mut self.rng, s.x, s.y, &[STAR, [1.0, 1.0, 1.0, 1.0]], SPARKS);
            }
        }
        if self.stars.iter().any(|s| !s.caught && s.y - RADIUS > WORLD_H) {
            Status::Lost
        } else if self.stars.iter().all(|s| s.caught) && self.sparks.is_empty() {
            Status::Won
        } else {
            Status::Running
        }
    }

    fn draw(&self, frame: &mut Frame) {
        frame.top.clear = [0.04, 0.05, 0.16, 1.0];
        frame.bottom.clear = [0.06, 0.08, 0.22, 1.0];
        for s in self.stars.iter().filter(|s| !s.caught) {
            frame.world_sprite(Sprite::Star, s.x, s.y, RADIUS * 2.6, s.y * 0.004, [1.0; 4]);
        }
        self.sparks.draw(frame);
        frame.bottom.text_centered(700.0, 8.0, [1.0, 1.0, 1.0, 0.5], "TAP THE STARS");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tapping_every_star_on_the_bottom_screen_wins() {
        let mut game = Stars::new(&mut Rng::new(7), 1.0);
        let mut status = Status::Running;
        for _ in 0..600 {
            let target = game.stars.iter().find(|s| !s.caught && s.y > BOTTOM_OFFSET + 100.0);
            let input = Input {
                touch: target.map(|s| [s.x, s.y - BOTTOM_OFFSET]),
                touch_started: target.is_some(),
                ..Default::default()
            };
            status = game.update(&input, 1.0 / 60.0, &mut Vec::new());
            if status != Status::Running {
                break;
            }
        }
        assert_eq!(status, Status::Won);
    }

    #[test]
    fn ignoring_the_stars_loses() {
        let mut game = Stars::new(&mut Rng::new(7), 1.0);
        let lost = (0..600).any(|_| game.update(&Input::default(), 1.0 / 60.0, &mut Vec::new()) == Status::Lost);
        assert!(lost);
    }
}
