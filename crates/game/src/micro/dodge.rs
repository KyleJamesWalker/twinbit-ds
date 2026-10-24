use super::{Microgame, Status};
use crate::{BOTTOM_OFFSET, Frame, Input, SCREEN_W, Sfx, Sprite, WORLD_H, rng::Rng};

const PLAYER_R: f32 = 28.0;
const PLAYER_Y: f32 = 660.0;
const MOVE: f32 = 560.0;
const ROCK_R: f32 = 40.0;
const FALL: f32 = 520.0;

struct Rock {
    x: f32,
    y: f32,
    delay: f32,
}

/// Rocks fall from the top screen; steer with the d-pad or stick on the bottom screen.
pub struct Dodge {
    x: f32,
    rocks: Vec<Rock>,
    speed: f32,
}

impl Dodge {
    pub fn new(rng: &mut Rng, speed: f32) -> Self {
        let rocks = (0..6)
            .map(|i| Rock {
                x: rng.range(ROCK_R, SCREEN_W - ROCK_R),
                y: -ROCK_R,
                delay: i as f32 * 0.55 / speed,
            })
            .collect();
        Self { x: SCREEN_W / 2.0, rocks, speed }
    }
}

impl Microgame for Dodge {
    fn prompt(&self) -> &'static str {
        "DODGE!"
    }

    fn duration(&self) -> f32 {
        5.5
    }

    fn win_on_timeout(&self) -> bool {
        true
    }

    fn update(&mut self, input: &Input, dt: f32, sfx: &mut Vec<Sfx>) -> Status {
        self.x = (self.x + input.dir()[0] * MOVE * dt).clamp(PLAYER_R, SCREEN_W - PLAYER_R);
        let py = PLAYER_Y + BOTTOM_OFFSET;
        for r in &mut self.rocks {
            if r.delay > 0.0 {
                r.delay -= dt;
            } else if r.y - ROCK_R < WORLD_H {
                r.y += FALL * self.speed * dt;
            }
        }
        let hit = self.rocks.iter().any(|r| (r.x - self.x).hypot(r.y - py) < ROCK_R + PLAYER_R);
        if hit {
            sfx.push(Sfx::Hit);
            Status::Lost
        } else {
            Status::Running
        }
    }

    fn draw(&self, frame: &mut Frame) {
        frame.top.clear = [0.18, 0.1, 0.08, 1.0];
        frame.bottom.clear = [0.24, 0.14, 0.1, 1.0];
        for r in &self.rocks {
            frame.world_sprite(Sprite::Rock, r.x, r.y, ROCK_R * 2.3, r.x * 0.01 + r.y * 0.003, [1.0; 4]);
        }
        frame.bottom.circle(self.x, PLAYER_Y, PLAYER_R, [0.3, 0.9, 1.0, 1.0]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standing_under_a_rock_loses() {
        let mut game = Dodge::new(&mut Rng::new(3), 1.0);
        game.x = game.rocks[0].x;
        let lost = (0..400).any(|_| game.update(&Input::default(), 1.0 / 60.0, &mut Vec::new()) == Status::Lost);
        assert!(lost);
    }
}
