mod balloon;
mod dodge;
mod stars;

use crate::{Frame, Input, Sfx, rng::Rng};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Running,
    Won,
    Lost,
}

pub trait Microgame {
    fn prompt(&self) -> &'static str;
    fn duration(&self) -> f32;
    fn update(&mut self, input: &Input, dt: f32, sfx: &mut Vec<Sfx>) -> Status;
    fn draw(&self, frame: &mut Frame);
    /// Survival games are won by outlasting the timer.
    fn win_on_timeout(&self) -> bool {
        false
    }
}

pub const KINDS: usize = 3;

pub fn make(kind: usize, rng: &mut Rng, speed: f32) -> Box<dyn Microgame> {
    match kind {
        0 => Box::new(stars::Stars::new(rng, speed)),
        1 => Box::new(balloon::Balloon::new(speed)),
        _ => Box::new(dodge::Dodge::new(rng, speed)),
    }
}
