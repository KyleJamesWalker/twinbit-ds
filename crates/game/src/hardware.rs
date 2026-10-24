//! Exercises each piece of hardware that is not a button: speakers, motor, LEDs, backlights.

use crate::save::LEVELS;
use crate::system::System;
use crate::{Align, Buttons, Frame, Input, SCREEN_W, Sfx};

const SHOWN: f32 = 0.6;
const TEXT: [f32; 4] = [1.0, 1.0, 1.0, 0.95];
const DIM: [f32; 4] = [1.0, 1.0, 1.0, 0.6];
const HOT: [f32; 4] = [1.0, 0.85, 0.2, 1.0];

/// State the hardware scene changes on the game, which owns the device outputs.
pub struct Controls<'a> {
    pub sounds: &'a mut Vec<Sfx>,
    pub rumble: &'a mut f32,
    pub brightness: &'a mut [u8; 2],
    pub leds: &'a mut [bool; 2],
}

#[derive(Default)]
pub struct Hardware {
    speaker: Option<(Sfx, f32)>,
}

impl Hardware {
    pub fn update(&mut self, input: &Input, dt: f32, c: Controls) {
        self.speaker = self.speaker.map(|(s, t)| (s, t - dt)).filter(|&(_, t)| t > 0.0);
        let fresh = input.pressed & !(Buttons::VOL_UP | Buttons::VOL_DOWN | Buttons::POWER | Buttons::BACK);
        let side = [(Buttons::L1, Sfx::Left), (Buttons::R1, Sfx::Right)].into_iter().find(|&(b, _)| fresh.any(b)).map(|(_, s)| s);
        if let Some(s) = side {
            c.sounds.push(s);
            self.speaker = Some((s, SHOWN));
        } else if fresh.0 != 0 {
            c.sounds.push(Sfx::Tick(fresh.0.trailing_zeros() as u8));
        }
        if fresh.any(Buttons::R2) {
            *c.rumble = 0.35;
        }
        if fresh.any(Buttons::X) {
            c.leds[0] = !c.leds[0];
        }
        if fresh.any(Buttons::B) {
            c.leds[1] = !c.leds[1];
        }
        let step = |up: Buttons, down: Buttons| fresh.contains(up) as i8 - fresh.contains(down) as i8;
        for (i, delta) in [step(Buttons::UP, Buttons::DOWN), step(Buttons::RIGHT, Buttons::LEFT)].into_iter().enumerate() {
            c.brightness[i] = c.brightness[i].saturating_add_signed(delta).clamp(1, LEVELS);
        }
    }

    pub fn draw(&self, f: &mut Frame, sys: &System, brightness: [u8; 2], leds: [bool; 2]) {
        let on = |b: bool| if b { "on" } else { "off" };
        let temp = |t: Option<f32>| t.map_or("--".to_string(), |t| format!("{t:.0} C"));
        let battery = sys.battery.map_or("--".to_string(), |b| format!("{}%{}", b.percent, if b.charging { ", charging" } else { "" }));
        let rows = [
            ("Top brightness", format!("{}/{LEVELS}", brightness[0])),
            ("Bottom brightness", format!("{}/{LEVELS}", brightness[1])),
            ("Work LED", on(leds[0]).to_string()),
            ("Low-power LED", on(leds[1]).to_string()),
            ("Headphones", if sys.headphones { "plugged in" } else { "out" }.to_string()),
            ("Battery", battery),
            ("SoC / GPU", format!("{} / {}", temp(sys.soc_c), temp(sys.gpu_c))),
        ];
        f.top.label("Hardware test", 48.0, 40.0, 48.0, HOT, Align::Left);
        for (i, (name, value)) in rows.into_iter().enumerate() {
            let y = 120.0 + i as f32 * 48.0;
            f.top.label(name, 64.0, y, 32.0, DIM, Align::Left);
            f.top.label(value, 440.0, y, 32.0, TEXT, Align::Left);
        }

        let help = [
            "Up / Down: top brightness",
            "Left / Right: bottom brightness",
            "X: work LED     B: low-power LED",
            "L1 / R1: left / right speaker",
            "R2: rumble     any button: tone",
            "Back: menu",
        ];
        for (i, line) in help.iter().enumerate() {
            f.bottom.label(*line, SCREEN_W / 2.0, 90.0 + i as f32 * 52.0, 32.0, DIM, Align::Center);
        }
        if let Some((side, _)) = self.speaker {
            let (x, label, align) = if side == Sfx::Left { (48.0, "LEFT", Align::Left) } else { (SCREEN_W - 48.0, "RIGHT", Align::Right) };
            f.bottom.label(label, x, 460.0, 64.0, HOT, align);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dpad_sets_each_screens_brightness_within_range() {
        let (mut sounds, mut rumble, mut brightness, mut leds) = (Vec::new(), 0.0, [LEVELS, 1], [false; 2]);
        let mut hw = Hardware::default();
        let press = Input { pressed: Buttons::UP | Buttons::LEFT | Buttons::X, ..Default::default() };
        hw.update(&press, 0.016, Controls { sounds: &mut sounds, rumble: &mut rumble, brightness: &mut brightness, leds: &mut leds });
        assert_eq!(brightness, [LEVELS, 1]);
        assert_eq!(leds, [true, false]);
        let press = Input { pressed: Buttons::DOWN | Buttons::RIGHT, ..Default::default() };
        hw.update(&press, 0.016, Controls { sounds: &mut sounds, rumble: &mut rumble, brightness: &mut brightness, leds: &mut leds });
        assert_eq!(brightness, [LEVELS - 1, 2]);
    }
}
