//! Button tester strip: every input's label, lit while held.

use crate::{Buttons, Canvas, Input, SCREEN_H, SCREEN_W, font};

const LABELS: [(&str, Buttons); 19] = [
    ("←", Buttons::LEFT),
    ("→", Buttons::RIGHT),
    ("↑", Buttons::UP),
    ("↓", Buttons::DOWN),
    ("A", Buttons::A),
    ("B", Buttons::B),
    ("X", Buttons::X),
    ("Y", Buttons::Y),
    ("L1", Buttons::L1),
    ("R1", Buttons::R1),
    ("L2", Buttons::L2),
    ("R2", Buttons::R2),
    ("SEL", Buttons::SELECT),
    ("STA", Buttons::START),
    ("HOME", Buttons::HOME),
    ("BACK", Buttons::BACK),
    ("V+", Buttons::VOL_UP),
    ("V-", Buttons::VOL_DOWN),
    ("PWR", Buttons::POWER),
];

const SCALE: f32 = 4.0;
const PAD: f32 = 4.0;
const GAP: f32 = 5.0;
const ON: [f32; 4] = [1.0, 0.85, 0.2, 1.0];
const OFF: [f32; 4] = [1.0, 1.0, 1.0, 0.25];

pub fn draw(c: &mut Canvas, input: &Input, rumble: bool, headphones: bool) {
    let (held, lid_closed) = (input.held, input.lid_closed);
    let chips = LABELS.iter().map(|&(l, b)| (l, held.contains(b))).chain([("LID", lid_closed), ("RUMBLE", rumble), ("HP", headphones)]);
    let width = |l: &str| font::width(l) * SCALE + 2.0 * PAD;
    let total: f32 = chips.clone().map(|(l, _)| width(l) + GAP).sum::<f32>() - GAP;
    let h = 5.0 * SCALE + 2.0 * PAD;
    let y = SCREEN_H - h - 8.0;
    let mut x = (SCREEN_W - total) / 2.0;
    c.rect(0.0, y - 6.0, SCREEN_W, h + 14.0, [0.0, 0.0, 0.0, 0.55]);
    for (label, on) in chips {
        let w = width(label);
        if on {
            c.rect(x, y, w, h, ON);
        }
        c.text(x + PAD, y + PAD, SCALE, if on { [0.0, 0.0, 0.0, 1.0] } else { OFF }, label);
        x += w + GAP;
    }

    let y = y - 6.0 - STICK_R - 16.0;
    stick(c, STICK_R + 24.0, y, "STICK", input.stick);
}

const STICK_R: f32 = 44.0;
const FINGER_R: f32 = 44.0;
const FINGERS: [[f32; 4]; 5] = [
    [1.0, 0.85, 0.2, 0.9],
    [0.3, 0.9, 1.0, 0.9],
    [1.0, 0.4, 0.6, 0.9],
    [0.5, 1.0, 0.4, 0.9],
    [0.8, 0.5, 1.0, 0.9],
];

/// A numbered ring under every finger on the bottom screen.
pub fn touches(c: &mut Canvas, input: &Input) {
    for (i, p) in input.touches.iter().enumerate() {
        let Some([x, y]) = *p else { continue };
        let color = FINGERS[i % FINGERS.len()];
        c.circle(x, y, FINGER_R, color);
        c.circle(x, y, FINGER_R - 8.0, [0.0, 0.0, 0.0, 0.6]);
        let n = (i + 1).to_string();
        c.text(x - font::width(&n) * 3.0, y - 7.5, 6.0, color, &n);
    }
}

fn stick(c: &mut Canvas, x: f32, y: f32, label: &str, [sx, sy]: [f32; 2]) {
    let moved = sx != 0.0 || sy != 0.0;
    c.circle(x, y, STICK_R, [0.0, 0.0, 0.0, 0.55]);
    c.circle(x, y, STICK_R - 4.0, [1.0, 1.0, 1.0, 0.12]);
    c.circle(x + sx * (STICK_R - 12.0), y + sy * (STICK_R - 12.0), 10.0, if moved { ON } else { OFF });
    c.text(x - font::width(label) * 2.0, y - STICK_R - 16.0, 4.0, if moved { ON } else { OFF }, label);
}
