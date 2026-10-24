use std::ops::{BitAnd, BitOr, BitOrAssign, Not};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Buttons(pub u32);

impl Buttons {
    pub const NONE: Self = Self(0);
    pub const UP: Self = Self(1 << 0);
    pub const DOWN: Self = Self(1 << 1);
    pub const LEFT: Self = Self(1 << 2);
    pub const RIGHT: Self = Self(1 << 3);
    pub const A: Self = Self(1 << 4);
    pub const B: Self = Self(1 << 5);
    pub const X: Self = Self(1 << 6);
    pub const Y: Self = Self(1 << 7);
    pub const L1: Self = Self(1 << 8);
    pub const R1: Self = Self(1 << 9);
    pub const START: Self = Self(1 << 10);
    pub const SELECT: Self = Self(1 << 11);
    pub const L2: Self = Self(1 << 12);
    pub const R2: Self = Self(1 << 13);
    pub const BACK: Self = Self(1 << 14);
    pub const HOME: Self = Self(1 << 15);
    pub const VOL_UP: Self = Self(1 << 16);
    pub const VOL_DOWN: Self = Self(1 << 17);
    pub const POWER: Self = Self(1 << 18);

    pub fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0 && other.0 != 0
    }

    pub fn any(self, other: Self) -> bool {
        self.0 & other.0 != 0
    }

    pub fn set(&mut self, other: Self, on: bool) {
        if on {
            self.0 |= other.0;
        } else {
            self.0 &= !other.0;
        }
    }
}

impl BitOr for Buttons {
    type Output = Self;
    fn bitor(self, rhs: Self) -> Self {
        Self(self.0 | rhs.0)
    }
}

impl BitOrAssign for Buttons {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

impl BitAnd for Buttons {
    type Output = Self;
    fn bitand(self, rhs: Self) -> Self {
        Self(self.0 & rhs.0)
    }
}

impl Not for Buttons {
    type Output = Self;
    fn not(self) -> Self {
        Self(!self.0)
    }
}

pub const MAX_TOUCHES: usize = 10;

/// Hardware state as a backend samples it each frame. Touch is in bottom-screen pixels.
/// `touch` is the first finger down; `touches` holds every finger.
#[derive(Clone, Copy, Debug, Default)]
pub struct RawInput {
    pub held: Buttons,
    pub stick: [f32; 2],
    pub touch: Option<[f32; 2]>,
    pub touches: [Option<[f32; 2]>; MAX_TOUCHES],
    pub mic: f32,
    pub lid_closed: bool,
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Input {
    pub held: Buttons,
    pub pressed: Buttons,
    pub stick: [f32; 2],
    pub touch: Option<[f32; 2]>,
    pub touches: [Option<[f32; 2]>; MAX_TOUCHES],
    pub touch_started: bool,
    pub mic: f32,
    pub lid_closed: bool,
}

impl Input {
    /// D-pad and stick combined, each axis in -1..=1.
    pub fn dir(&self) -> [f32; 2] {
        let axis = |neg, pos, stick: f32| {
            let pad = self.held.contains(pos) as i32 - self.held.contains(neg) as i32;
            (pad as f32 + stick).clamp(-1.0, 1.0)
        };
        [
            axis(Buttons::LEFT, Buttons::RIGHT, self.stick[0]),
            axis(Buttons::UP, Buttons::DOWN, self.stick[1]),
        ]
    }
}

#[derive(Default)]
pub struct InputTracker {
    prev: RawInput,
}

impl InputTracker {
    pub fn frame(&mut self, raw: RawInput) -> Input {
        let input = Input {
            held: raw.held,
            pressed: raw.held & !self.prev.held,
            stick: raw.stick,
            touch: raw.touch,
            touches: raw.touches,
            touch_started: raw.touch.is_some() && self.prev.touch.is_none(),
            mic: raw.mic,
            lid_closed: raw.lid_closed,
        };
        self.prev = raw;
        input
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn edges_fire_once() {
        let mut t = InputTracker::default();
        let raw = RawInput { held: Buttons::A, touch: Some([1.0, 2.0]), ..Default::default() };
        let first = t.frame(raw);
        assert!(first.pressed.contains(Buttons::A));
        assert!(first.touch_started);
        let second = t.frame(raw);
        assert_eq!(second.pressed, Buttons::NONE);
        assert!(!second.touch_started);
    }
}
