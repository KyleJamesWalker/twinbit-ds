use super::{Microgame, Status};
use crate::{Frame, Input, SCREEN_W, Sfx};

const START: f32 = 50.0;
const POP: f32 = 230.0;
const INFLATE: f32 = 110.0;
const LEAK: f32 = 20.0;
/// Mic RMS below this is room noise; at FULL and above the balloon inflates at full rate.
const NOISE: f32 = 0.05;
const FULL: f32 = 0.3;

/// Blow into the hinge mic to inflate the balloon on the top screen until it pops.
pub struct Balloon {
    r: f32,
    level: f32,
    speed: f32,
}

impl Balloon {
    pub fn new(speed: f32) -> Self {
        Self { r: START, level: 0.0, speed }
    }
}

impl Microgame for Balloon {
    fn prompt(&self) -> &'static str {
        "BLOW!"
    }

    fn duration(&self) -> f32 {
        5.0 / self.speed.sqrt()
    }

    fn update(&mut self, input: &Input, dt: f32, sfx: &mut Vec<Sfx>) -> Status {
        self.level = ((input.mic - NOISE) / (FULL - NOISE)).clamp(0.0, 1.0);
        self.r = (self.r + (self.level * INFLATE - LEAK) * dt).max(START);
        if self.r >= POP {
            sfx.push(Sfx::Pop);
            Status::Won
        } else {
            Status::Running
        }
    }

    fn draw(&self, frame: &mut Frame) {
        frame.top.clear = [0.55, 0.8, 0.95, 1.0];
        frame.bottom.clear = [0.2, 0.22, 0.26, 1.0];
        let cy = 400.0;
        frame.top.rect(SCREEN_W / 2.0 - 2.0, cy + self.r, 4.0, 768.0, [0.2, 0.2, 0.2, 1.0]);
        frame.top.circle(SCREEN_W / 2.0, cy, self.r, [0.9, 0.15, 0.2, 1.0]);

        frame.bottom.text_centered(140.0, 10.0, [1.0, 1.0, 1.0, 1.0], "BLOW INTO THE HINGE");
        let (x, y, w, h) = (112.0, 360.0, 800.0, 80.0);
        frame.bottom.rect(x, y, w, h, [0.1, 0.1, 0.12, 1.0]);
        frame.bottom.rect(x, y, w * self.level, h, [0.3, 0.9, 0.4, 1.0]);
        frame.bottom.rect(x + w * (self.r - START) / (POP - START) - 3.0, y - 20.0, 6.0, h + 40.0, [1.0, 1.0, 1.0, 1.0]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sustained_blowing_pops_and_silence_does_not() {
        let blow = Input { mic: 1.0, ..Default::default() };
        let mut game = Balloon::new(1.0);
        let popped = (0..300).any(|_| game.update(&blow, 1.0 / 60.0, &mut Vec::new()) == Status::Won);
        assert!(popped);

        let mut game = Balloon::new(1.0);
        let popped = (0..300).any(|_| game.update(&Input::default(), 1.0 / 60.0, &mut Vec::new()) == Status::Won);
        assert!(!popped);
    }
}
