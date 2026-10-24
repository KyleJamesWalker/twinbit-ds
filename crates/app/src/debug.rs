//! Remote control for development: commands arrive one per line on a named pipe.
//!
//! ```text
//! press <button> [ms]          hold a button (a b x y l1 r1 l2 r2 start select home back up down left right volup voldown)
//! tap <x> <y>                  touch the bottom screen at screen pixels
//! swipe <x1> <y1> <x2> <y2> [ms]
//! scene <name>                 jump to a scene: micro, stars, balloon, dodge, dice, stress, hardware
//! sleep <secs>                 suspend, waking on the RTC alarm
//! panic                        crash on purpose to test the crash log
//! shot <path>                  save both screens, top above bottom, as one PNG
//! ```

use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, channel};

use twinbit_game::Buttons;

pub const PIPE: &str = "/tmp/twinbit.ctl";
const PRESS_MS: f32 = 120.0;
const SWIPE_MS: f32 = 200.0;
const TAP_S: f32 = 0.08;

pub enum Command {
    Press(Buttons, f32),
    Touch(Vec<(f32, Option<[f32; 2]>)>),
    Scene(String),
    Shot(PathBuf),
    Sleep(u32),
    Panic,
}

pub fn listen() -> Option<Receiver<Command>> {
    std::env::var_os("TWINBIT_DEBUG")?;
    let _ = std::fs::remove_file(PIPE);
    if !std::process::Command::new("mkfifo").arg(PIPE).status().is_ok_and(|s| s.success()) {
        eprintln!("debug: mkfifo {PIPE} failed");
        return None;
    }
    eprintln!("debug: listening on {PIPE}");
    let (tx, rx) = channel();
    std::thread::spawn(move || {
        // Opening a pipe for reading blocks until a writer connects; each writer's EOF reopens it.
        while let Ok(f) = File::open(PIPE) {
            for line in BufReader::new(f).lines().map_while(Result::ok) {
                match parse(&line) {
                    Some(cmd) => {
                        eprintln!("debug: {line}");
                        if tx.send(cmd).is_err() {
                            return;
                        }
                    }
                    None => eprintln!("debug: bad command {line:?}"),
                }
            }
        }
    });
    Some(rx)
}

fn parse(line: &str) -> Option<Command> {
    let words: Vec<&str> = line.split_whitespace().collect();
    let num = |i: usize| words.get(i).and_then(|w| w.parse::<f32>().ok());
    Some(match *words.first()? {
        "press" => Command::Press(button(words.get(1)?)?, num(2).unwrap_or(PRESS_MS) / 1000.0),
        "tap" => {
            let p = [num(1)?, num(2)?];
            Command::Touch(vec![(0.0, Some(p)), (TAP_S, None)])
        }
        "swipe" => {
            let (a, b) = ([num(1)?, num(2)?], [num(3)?, num(4)?]);
            let dur = num(5).unwrap_or(SWIPE_MS) / 1000.0;
            let mut keys: Vec<_> = (0..=10)
                .map(|i| {
                    let k = i as f32 / 10.0;
                    (dur * k, Some([a[0] + (b[0] - a[0]) * k, a[1] + (b[1] - a[1]) * k]))
                })
                .collect();
            keys.push((dur + 1.0 / 60.0, None));
            Command::Touch(keys)
        }
        "scene" => Command::Scene(words.get(1)?.to_string()),
        "shot" => Command::Shot(PathBuf::from(words.get(1)?)),
        "sleep" => Command::Sleep(num(1)? as u32),
        "panic" => Command::Panic,
        _ => return None,
    })
}

fn button(name: &str) -> Option<Buttons> {
    Some(match name {
        "a" => Buttons::A,
        "b" => Buttons::B,
        "x" => Buttons::X,
        "y" => Buttons::Y,
        "l1" => Buttons::L1,
        "r1" => Buttons::R1,
        "l2" => Buttons::L2,
        "r2" => Buttons::R2,
        "start" => Buttons::START,
        "select" => Buttons::SELECT,
        "home" => Buttons::HOME,
        "back" => Buttons::BACK,
        "up" => Buttons::UP,
        "down" => Buttons::DOWN,
        "left" => Buttons::LEFT,
        "right" => Buttons::RIGHT,
        "volup" => Buttons::VOL_UP,
        "voldown" => Buttons::VOL_DOWN,
        _ => return None,
    })
}

/// Injected input that plays out over the next frames.
#[derive(Default)]
pub struct Injector {
    presses: Vec<(Buttons, f32)>,
    touch: Vec<(f32, Option<[f32; 2]>)>,
    t: f32,
}

impl Injector {
    pub fn press(&mut self, b: Buttons, secs: f32) {
        self.presses.push((b, secs));
    }

    pub fn touch(&mut self, keys: Vec<(f32, Option<[f32; 2]>)>) {
        self.touch = keys;
        self.t = 0.0;
    }

    /// Advances by `dt` and returns the buttons held and the touch point for this frame.
    pub fn frame(&mut self, dt: f32) -> (Buttons, Option<Option<[f32; 2]>>) {
        let mut held = Buttons::NONE;
        for (b, left) in &mut self.presses {
            held |= *b;
            *left -= dt;
        }
        self.presses.retain(|&(_, left)| left > 0.0);

        let touch = self.touch.iter().rev().find(|(at, _)| *at <= self.t).map(|&(_, p)| p);
        if self.touch.last().is_some_and(|&(at, _)| self.t > at) {
            self.touch.clear();
        }
        self.t += dt;
        (held, touch)
    }
}

pub fn save_png(path: &std::path::Path, width: u32, height: u32, rgba: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    let tmp = path.with_extension("tmp");
    let mut enc = png::Encoder::new(File::create(&tmp)?, width, height);
    enc.set_color(png::ColorType::Rgba);
    enc.set_depth(png::BitDepth::Eight);
    enc.write_header()?.write_image_data(rgba)?;
    std::fs::rename(tmp, path)?;
    Ok(())
}
