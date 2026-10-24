//! What the backend reports about the device each frame, and what the game asks of it.

use crate::{Align, Canvas, SCREEN_W};

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Battery {
    pub percent: u8,
    pub charging: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct System {
    pub fps: f32,
    pub frame_ms: f32,
    /// Longest frame in the last second.
    pub worst_ms: f32,
    pub soc_c: Option<f32>,
    pub gpu_c: Option<f32>,
    pub battery: Option<Battery>,
    pub headphones: bool,
}

/// Device state the game controls. Brightness runs 0..=1 per screen, top first.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Outputs {
    pub rumble: bool,
    pub volume: f32,
    pub brightness: [f32; 2],
    pub screens_on: bool,
    pub work_led: bool,
    pub low_led: bool,
}

const PANEL: [f32; 4] = [0.0, 0.0, 0.0, 0.6];
const TEXT: [f32; 4] = [1.0, 1.0, 1.0, 0.95];
const WARN: [f32; 4] = [1.0, 0.45, 0.35, 1.0];
const SIZE: f32 = 22.0;

/// FPS, frame times and temperatures in the top-right corner.
pub fn draw_perf(c: &mut Canvas, s: &System) {
    let temp = |t: Option<f32>| t.map_or("--".to_string(), |t| format!("{t:.0} C"));
    let lines = [
        (format!("{:.0} FPS", s.fps), if s.fps < 55.0 { WARN } else { TEXT }),
        (format!("{:.1} ms  worst {:.1}", s.frame_ms, s.worst_ms), if s.worst_ms > 20.0 { WARN } else { TEXT }),
        (format!("SoC {}  GPU {}", temp(s.soc_c), temp(s.gpu_c)), TEXT),
    ];
    let (w, x) = (300.0, SCREEN_W - 12.0);
    c.rect(x - w - 8.0, 8.0, w + 16.0, lines.len() as f32 * (SIZE + 4.0) + 12.0, PANEL);
    for (i, (line, color)) in lines.into_iter().enumerate() {
        c.label(line, x, 14.0 + i as f32 * (SIZE + 4.0), SIZE, color, Align::Right);
    }
}

/// Battery level in the top-right corner of a screen.
pub fn draw_battery(c: &mut Canvas, s: &System, y: f32) {
    let Some(b) = s.battery else { return };
    let color = if b.percent <= 15 && !b.charging { WARN } else { TEXT };
    let text = if b.charging { format!("{}% charging", b.percent) } else { format!("{}%", b.percent) };
    c.label(text, SCREEN_W - 16.0, y, SIZE, color, Align::Right);
}
