//! RG DS Plus hardware that Weston does not forward: the gamepad, the Back and power keys, the lid sensor, the motor.

use std::fs;
use std::sync::{Arc, Mutex};

use evdev::{AbsoluteAxisCode, EventSummary, KeyCode};
use twinbit_game::Buttons;

const PAD_NAME: &str = "ANBERNIC-rk3568-keys";
const BACK_NAME: &str = "adc-keys";
const POWER_NAME: &str = "rk805 pwrkey";
const HALL: &str = "/sys/class/anbernic_misc/hallkey";
const MOTOR: &str = "/sys/class/anbernic_misc/moto";
const SUSPEND: &str = "/mnt/vendor/ctrl/pwr_new.sh";
const STICK_MAX: f32 = 4096.0;

#[derive(Clone, Copy, Default)]
struct Pad {
    held: Buttons,
    hat: Buttons,
    stick: [f32; 2],
}

pub struct Device {
    pad: Arc<Mutex<Pad>>,
    hall_open: Option<String>,
    rumbling: bool,
}

impl Device {
    pub fn open() -> Option<Self> {
        let log = std::env::var_os("TWINBIT_LOG_INPUT").is_some();
        let pad = Arc::new(Mutex::new(Pad::default()));
        let mut found = false;
        for (path, mut dev) in evdev::enumerate().filter(|(_, d)| matches!(d.name(), Some(PAD_NAME | BACK_NAME | POWER_NAME))) {
            eprintln!("input: {} {:?}", path.display(), dev.name());
            found = true;
            let p = pad.clone();
            std::thread::spawn(move || {
                while let Ok(events) = dev.fetch_events() {
                    for ev in events {
                        let summary = ev.destructure();
                        if log && !matches!(summary, EventSummary::Synchronization(..)) {
                            eprintln!("input: {summary:?}");
                        }
                        handle(&mut p.lock().unwrap(), summary);
                    }
                }
            });
        }
        let hall_open = fs::read_to_string(HALL).ok();
        found.then_some(Self { pad, hall_open, rumbling: false })
    }

    pub fn held(&self) -> Buttons {
        let pad = self.pad.lock().unwrap();
        pad.held | pad.hat
    }

    pub fn stick(&self) -> [f32; 2] {
        let s = self.pad.lock().unwrap().stick;
        s.map(|v| if v.abs() < 0.15 { 0.0 } else { v })
    }

    pub fn set_rumble(&mut self, on: bool) {
        if on != self.rumbling {
            self.rumbling = on;
            if let Err(e) = fs::write(MOTOR, if on { "1" } else { "0" }) {
                eprintln!("motor: {e}");
            }
        }
    }

    /// Suspends through the stock script, which returns once the device wakes.
    pub fn suspend(&mut self) {
        self.set_rumble(false);
        if let Err(e) = std::process::Command::new(SUSPEND).status() {
            eprintln!("suspend: {e}");
        }
    }

    /// The hall sensor's reading at startup, when the lid is necessarily open, is taken as "open".
    pub fn lid_closed(&self) -> bool {
        match &self.hall_open {
            Some(open) => fs::read_to_string(HALL).is_ok_and(|now| &now != open),
            None => false,
        }
    }
}

impl Drop for Device {
    fn drop(&mut self) {
        self.set_rumble(false);
    }
}

fn handle(pad: &mut Pad, event: EventSummary) {
    match event {
        EventSummary::Key(_, code, value) => {
            if let Some(b) = button(code) {
                pad.held.set(b, value != 0);
            }
        }
        EventSummary::AbsoluteAxis(_, axis, value) => match axis {
            AbsoluteAxisCode::ABS_HAT0X => {
                pad.hat.set(Buttons::LEFT, value < 0);
                pad.hat.set(Buttons::RIGHT, value > 0);
            }
            AbsoluteAxisCode::ABS_HAT0Y => {
                pad.hat.set(Buttons::UP, value < 0);
                pad.hat.set(Buttons::DOWN, value > 0);
            }
            AbsoluteAxisCode::ABS_Z => pad.stick[0] = value as f32 / STICK_MAX,
            AbsoluteAxisCode::ABS_RX => pad.stick[1] = value as f32 / STICK_MAX,
            _ => {}
        },
        _ => {}
    }
}

/// Stock firmware codes, mapped by pressing each labelled button; they do not follow the evdev gamepad spec.
fn button(code: KeyCode) -> Option<Buttons> {
    Some(match code {
        KeyCode::BTN_SOUTH => Buttons::A,
        KeyCode::BTN_EAST => Buttons::B,
        KeyCode::BTN_NORTH => Buttons::X,
        KeyCode::BTN_C => Buttons::Y,
        KeyCode::BTN_WEST => Buttons::L1,
        KeyCode::BTN_Z => Buttons::R1,
        KeyCode::BTN_SELECT => Buttons::L2,
        KeyCode::BTN_START => Buttons::R2,
        KeyCode::BTN_TL => Buttons::SELECT,
        KeyCode::BTN_TR => Buttons::START,
        KeyCode::BTN_TL2 => Buttons::HOME,
        KeyCode::KEY_BACK => Buttons::BACK,
        KeyCode::KEY_VOLUMEUP => Buttons::VOL_UP,
        KeyCode::KEY_VOLUMEDOWN => Buttons::VOL_DOWN,
        KeyCode::KEY_POWER => Buttons::POWER,
        _ => return None,
    })
}
