//! Spawns bouncing textured sprites on each screen to find where the frame rate drops.

use crate::{Align, Buttons, Canvas, Frame, Input, SCREEN_H, SCREEN_W, Sfx, Sprite, rng::Rng};

/// Spawn rates in sprites per second; L1/R1 and L2/R2 step through them.
const RATES: [u32; 11] = [0, 1, 5, 10, 25, 50, 100, 250, 500, 1000, 2500];
const MAX_SPRITES: usize = 50_000;
const TEXT: [f32; 4] = [1.0, 1.0, 1.0, 0.95];

struct Item {
    x: f32,
    y: f32,
    vx: f32,
    vy: f32,
    angle: f32,
    spin: f32,
    size: f32,
    sprite: Sprite,
}

#[derive(Default)]
struct Field {
    items: Vec<Item>,
    rate: usize,
    owed: f32,
}

impl Field {
    fn update(&mut self, rng: &mut Rng, dt: f32) {
        self.owed += RATES[self.rate] as f32 * dt;
        while self.owed >= 1.0 && self.items.len() < MAX_SPRITES {
            self.owed -= 1.0;
            let size = rng.range(24.0, 56.0);
            let (speed, dir) = (rng.range(60.0, 260.0), rng.range(0.0, std::f32::consts::TAU));
            self.items.push(Item {
                x: rng.range(size, SCREEN_W - size),
                y: rng.range(size, SCREEN_H - size),
                vx: dir.cos() * speed,
                vy: dir.sin() * speed,
                angle: rng.range(0.0, std::f32::consts::TAU),
                spin: rng.range(-3.0, 3.0),
                size,
                sprite: Sprite::ALL[rng.below(Sprite::ALL.len())],
            });
        }
        self.owed = self.owed.min(1.0);
        for it in &mut self.items {
            it.x += it.vx * dt;
            it.y += it.vy * dt;
            it.angle += it.spin * dt;
            let r = it.size / 2.0;
            if it.x < r || it.x > SCREEN_W - r {
                it.vx = -it.vx;
                it.x = it.x.clamp(r, SCREEN_W - r);
            }
            if it.y < r || it.y > SCREEN_H - r {
                it.vy = -it.vy;
                it.y = it.y.clamp(r, SCREEN_H - r);
            }
        }
    }

    fn draw(&self, c: &mut Canvas, name: &str, keys: &str) {
        for it in &self.items {
            c.sprite(it.sprite, it.x, it.y, it.size, it.angle, [1.0; 4]);
        }
        c.rect(0.0, 0.0, 560.0, 84.0, [0.0, 0.0, 0.0, 0.6]);
        c.label(format!("{name}: {} sprites", self.items.len()), 16.0, 10.0, 30.0, TEXT, Align::Left);
        c.label(format!("+{}/s   {keys}", RATES[self.rate]), 16.0, 46.0, 24.0, TEXT, Align::Left);
    }
}

pub struct Stress {
    fields: [Field; 2],
    rng: Rng,
}

impl Stress {
    pub fn new(rng: &mut Rng) -> Self {
        let mut s = Self { fields: Default::default(), rng: Rng::new(rng.next_u32() as u64) };
        s.fields[0].rate = 6;
        s.fields[1].rate = 6;
        s
    }

    pub fn update(&mut self, input: &Input, dt: f32, sfx: &mut Vec<Sfx>) {
        let steps = [(0, Buttons::L1, Buttons::R1), (1, Buttons::L2, Buttons::R2)];
        for (i, down, up) in steps {
            let f = &mut self.fields[i];
            let before = f.rate;
            if input.pressed.contains(down) {
                f.rate = f.rate.saturating_sub(1);
            }
            if input.pressed.contains(up) {
                f.rate = (f.rate + 1).min(RATES.len() - 1);
            }
            if f.rate != before {
                sfx.push(Sfx::Tick(f.rate as u8 * 2));
            }
        }
        if input.pressed.contains(Buttons::A) {
            self.fields.iter_mut().for_each(|f| f.items.clear());
            sfx.push(Sfx::Pop);
        }
        for f in &mut self.fields {
            f.update(&mut self.rng, dt);
        }
    }

    pub fn draw(&self, frame: &mut Frame) {
        frame.top.clear = [0.05, 0.06, 0.1, 1.0];
        frame.bottom.clear = [0.07, 0.05, 0.1, 1.0];
        self.fields[0].draw(&mut frame.top, "TOP", "L1 slower  R1 faster");
        self.fields[1].draw(&mut frame.bottom, "BOTTOM", "L2 slower  R2 faster");
        frame.bottom.label("A: clear both   BACK: menu", SCREEN_W / 2.0, SCREEN_H - 40.0, 24.0, TEXT, Align::Center);
    }

    pub fn counts(&self) -> [usize; 2] {
        [self.fields[0].items.len(), self.fields[1].items.len()]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rates_step_and_spawn_per_screen() {
        let mut s = Stress::new(&mut Rng::new(1));
        let up_top = Input { pressed: Buttons::R1, ..Default::default() };
        s.update(&up_top, 0.0, &mut Vec::new());
        assert_eq!(RATES[s.fields[0].rate], 250);
        assert_eq!(RATES[s.fields[1].rate], 100);
        for _ in 0..60 {
            s.update(&Input::default(), 1.0 / 60.0, &mut Vec::new());
        }
        let [top, bottom] = s.counts();
        assert!((245..=251).contains(&top), "top spawned {top}");
        assert!((98..=101).contains(&bottom), "bottom spawned {bottom}");
        s.update(&Input { pressed: Buttons::A, ..Default::default() }, 0.0, &mut Vec::new());
        assert_eq!(s.counts(), [0, 0]);
    }
}
