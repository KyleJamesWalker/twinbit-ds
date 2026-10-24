//! RG DS Plus hardware outside Wayland: the gamepad, the Back and power keys, the headphone jack, sysfs sensors and outputs.

use std::fs;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

use evdev::{AbsoluteAxisCode, EventSummary, KeyCode};
use twinbit_game::{Battery, Buttons, Outputs};

const PAD_NAME: &str = "ANBERNIC-rk3568-keys";
const BACK_NAME: &str = "adc-keys";
const POWER_NAME: &str = "rk805 pwrkey";
const HALL: &str = "/sys/class/anbernic_misc/hallkey";
const MOTOR: &str = "/sys/class/anbernic_misc/moto";
const SUSPEND: &str = "/mnt/vendor/ctrl/pwr_new.sh";
/// The codec reports the jack only through extcon; its input switch device never sends events.
const JACK_EXTCON: &str = "rk817-sound";
const JACK_POLL: std::time::Duration = std::time::Duration::from_millis(250);
/// PulseAudio sinks from the stock `setPulseaudio.sh`: the speaker amp, and the codec that drives the jack.
const SPEAKER_SINK: &str = "alsa_output.1.stereo-fallback";
const HEADPHONE_SINK: &str = "alsa_output.0.HiFi__Speaker__sink";
const WAKE_ALARM: &str = "/sys/class/rtc/rtc0/wakealarm";
/// Top screen first.
const BACKLIGHTS: [&str; 2] = ["/sys/class/backlight/backlight", "/sys/class/backlight/backlight1"];
const BACKLIGHT_MIN: f32 = 8.0;
const WORK_LED: &str = "/sys/class/anbernic_misc/work_led";
const LOW_LED: &str = "/sys/class/anbernic_misc/lowpwr_led";
/// The stock menu sets this to 1 when it launches a port, and Weston then delivers no touches.
const TOUCH_CTRL: &str = "/sys/class/anbernic_misc/tpctrl";
/// The startup scripts select the cw2015 fuel gauge when present; the PMU's own estimate disagrees with it.
const BATTERIES: [&str; 2] = ["/sys/class/power_supply/cw2015-battery", "/sys/class/power_supply/battery"];
const STICK_MAX: f32 = 4096.0;

#[derive(Clone, Copy, Default)]
struct Pad {
    held: Buttons,
    hat: Buttons,
    stick: [f32; 2],
}

/// Sysfs output values as last written, so each is only touched when it changes.
#[derive(Clone, Copy, PartialEq)]
struct Written {
    brightness: [u32; 2],
    screens_on: bool,
    leds: [bool; 2],
}

pub struct Device {
    pad: Arc<Mutex<Pad>>,
    headphones: Arc<AtomicBool>,
    hall_open: Option<String>,
    rumbling: bool,
    original_brightness: [Option<String>; 2],
    original_touch: Option<String>,
    written: Option<Written>,
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
        let headphones = Arc::new(AtomicBool::new(false));
        if let Some(state) = jack_state_path() {
            let hp = headphones.clone();
            std::thread::spawn(move || {
                let mut last = None;
                loop {
                    let now = fs::read_to_string(&state).is_ok_and(|s| s.contains("HEADPHONE=1"));
                    if last != Some(now) {
                        eprintln!("headphones: {}", if now { "in" } else { "out" });
                        hp.store(now, Ordering::Relaxed);
                        route_audio(if now { HEADPHONE_SINK } else { SPEAKER_SINK });
                        last = Some(now);
                    }
                    std::thread::sleep(JACK_POLL);
                }
            });
        }
        let hall_open = fs::read_to_string(HALL).ok();
        let original_brightness = BACKLIGHTS.map(|b| fs::read_to_string(format!("{b}/brightness")).ok());
        let original_touch = read_trim(TOUCH_CTRL);
        if original_touch.as_deref().is_some_and(|t| t != "0") {
            eprintln!("touch: tpctrl was {}, setting 0", original_touch.as_deref().unwrap_or_default());
            write(TOUCH_CTRL, "0");
        }
        found.then_some(Self { pad, headphones, hall_open, rumbling: false, original_brightness, original_touch, written: None })
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
    /// With `wake_after`, the RTC alarm wakes it without a key press.
    pub fn suspend(&mut self, wake_after: Option<u32>) {
        self.set_rumble(false);
        if let Some(secs) = wake_after {
            let armed = fs::write(WAKE_ALARM, "0").and_then(|_| fs::write(WAKE_ALARM, format!("+{secs}")));
            if let Err(e) = armed {
                eprintln!("suspend: wake alarm: {e}");
                return;
            }
        }
        if let Err(e) = std::process::Command::new(SUSPEND).status() {
            eprintln!("suspend: {e}");
        }
    }

    pub fn headphones(&self) -> bool {
        self.headphones.load(Ordering::Relaxed)
    }

    pub fn battery(&self) -> Option<Battery> {
        let dir = BATTERIES.iter().find(|d| fs::metadata(format!("{d}/capacity")).is_ok())?;
        let percent = read_trim(&format!("{dir}/capacity"))?.parse().ok()?;
        let charging = read_trim(&format!("{dir}/status")).is_some_and(|s| s == "Charging" || s == "Full");
        Some(Battery { percent, charging })
    }

    /// Temperature in Celsius of the thermal zone whose `type` is `kind`.
    pub fn temperature(&self, kind: &str) -> Option<f32> {
        let zone = fs::read_dir("/sys/class/thermal").ok()?.flatten().find(|z| read_trim(&format!("{}/type", z.path().display())).as_deref() == Some(kind))?;
        read_trim(&format!("{}/temp", zone.path().display()))?.parse::<f32>().ok().map(|m| m / 1000.0)
    }

    pub fn apply(&mut self, out: &Outputs) {
        self.set_rumble(out.rumble);
        let want = Written {
            brightness: out.brightness.map(|b| (b * 255.0).round().max(BACKLIGHT_MIN) as u32),
            screens_on: out.screens_on,
            leds: [out.work_led, out.low_led],
        };
        let old = self.written.replace(want);
        let changed = |f: fn(&Written) -> bool| old.as_ref().is_none_or(|o| f(o) != f(&want));
        for (i, path) in BACKLIGHTS.iter().enumerate() {
            if old.is_none_or(|o| o.brightness[i] != want.brightness[i]) {
                write(&format!("{path}/brightness"), &want.brightness[i].to_string());
            }
            if changed(|w| w.screens_on) {
                // 4 is FB_BLANK_POWERDOWN.
                write(&format!("{path}/bl_power"), if want.screens_on { "0" } else { "4" });
            }
        }
        if changed(|w| w.leds[0]) {
            write(WORK_LED, if want.leds[0] { "1" } else { "0" });
        }
        if changed(|w| w.leds[1]) {
            write(LOW_LED, if want.leds[1] { "1" } else { "0" });
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
    /// Hands the backlights, LEDs and touch mode back the way the frontend left them.
    fn drop(&mut self) {
        self.set_rumble(false);
        for (path, original) in BACKLIGHTS.iter().zip(&self.original_brightness) {
            write(&format!("{path}/bl_power"), "0");
            if let Some(b) = original {
                write(&format!("{path}/brightness"), b.trim());
            }
        }
        write(WORK_LED, "0");
        write(LOW_LED, "0");
        if let Some(t) = &self.original_touch {
            write(TOUCH_CTRL, t);
        }
    }
}

fn jack_state_path() -> Option<std::path::PathBuf> {
    fs::read_dir("/sys/class/extcon").ok()?.flatten().map(|e| e.path()).find(|p| read_trim(&format!("{}/name", p.display())).as_deref() == Some(JACK_EXTCON)).map(|p| p.join("state"))
}

/// Makes `sink` the default and moves every playing stream onto it, since a new default only affects new streams.
fn route_audio(sink: &str) {
    let pactl = |args: &[&str]| std::process::Command::new("pactl").args(args).output();
    if let Err(e) = pactl(&["set-default-sink", sink]) {
        eprintln!("audio route: {e}");
        return;
    }
    let Ok(out) = pactl(&["list", "short", "sink-inputs"]) else { return };
    for id in String::from_utf8_lossy(&out.stdout).lines().filter_map(|l| l.split_whitespace().next()) {
        pactl(&["move-sink-input", id, sink]).ok();
    }
}

fn read_trim(path: &str) -> Option<String> {
    fs::read_to_string(path).ok().map(|s| s.trim().to_string())
}

fn write(path: &str, value: &str) {
    if let Err(e) = fs::write(path, value) {
        eprintln!("{path}: {e}");
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
