mod audio;
mod debug;
#[cfg(target_os = "linux")]
mod device;
mod gfx;
mod mic;

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use twinbit_game::{Buttons, Game, InputTracker, MAX_TOUCHES, RawInput, SCREEN_H, SCREEN_W};
use winit::application::ApplicationHandler;
use winit::dpi::{PhysicalPosition, PhysicalSize};
use winit::event::{ElementState, MouseButton, TouchPhase, WindowEvent};
use winit::event_loop::{ActiveEventLoop, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::WindowId;

const QUIT_HOLD: f32 = 1.0;
/// The press that wakes the device arrives after resume and must not suspend it again.
#[cfg_attr(not(target_os = "linux"), allow(dead_code))]
const WAKE_GRACE: Duration = Duration::from_secs(2);

struct App {
    game: Game,
    tracker: InputTracker,
    gfx: Option<gfx::Gfx>,
    mic: Option<mic::Mic>,
    audio: Option<audio::Audio>,
    #[cfg(target_os = "linux")]
    device: Option<device::Device>,
    keys: Buttons,
    lid_key: bool,
    blow_key: bool,
    cursor: Option<[f32; 2]>,
    mouse_down: bool,
    touches: Vec<(u64, [f32; 2])>,
    home_held: f32,
    last: Instant,
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    woke: Instant,
    debug: Option<std::sync::mpsc::Receiver<debug::Command>>,
    injector: debug::Injector,
}

impl App {
    fn new() -> Self {
        let seed = SystemTime::now().duration_since(UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64);
        let mut game = Game::new(seed);
        if std::env::var("TWINBIT_START").as_deref() == Ok("dice") {
            game.open_dice();
        }
        Self {
            game,
            tracker: InputTracker::default(),
            gfx: None,
            mic: mic::Mic::open().inspect_err(|e| eprintln!("mic disabled: {e}")).ok(),
            audio: audio::Audio::open().inspect_err(|e| eprintln!("audio disabled: {e}")).ok(),
            #[cfg(target_os = "linux")]
            device: device::Device::open(),
            keys: Buttons::NONE,
            lid_key: false,
            blow_key: false,
            cursor: None,
            mouse_down: false,
            touches: Vec::new(),
            home_held: 0.0,
            last: Instant::now(),
            woke: Instant::now(),
            debug: debug::listen(),
            injector: debug::Injector::default(),
        }
    }

    fn raw_input(&self) -> RawInput {
        #[cfg_attr(not(target_os = "linux"), allow(unused_mut))]
        let mut raw = RawInput {
            held: self.keys,
            touch: self.touches.first().map(|&(_, p)| p).or(self.cursor.filter(|_| self.mouse_down)),
            mic: self.mic.as_ref().map_or(0.0, |m| m.level()).max(if self.blow_key { 1.0 } else { 0.0 }),
            lid_closed: self.lid_key,
            ..Default::default()
        };
        for (slot, &(_, p)) in raw.touches.iter_mut().zip(&self.touches) {
            *slot = Some(p);
        }
        if self.touches.is_empty() {
            raw.touches[0] = raw.touch;
        }
        #[cfg(target_os = "linux")]
        if let Some(dev) = &self.device {
            raw.held |= dev.held();
            raw.stick = dev.stick();
            raw.lid_closed |= dev.lid_closed();
        }
        raw
    }

    fn tick(&mut self, el: &ActiveEventLoop) {
        let now = Instant::now();
        let dt = (now - self.last).as_secs_f32().min(0.05);
        self.last = now;
        let mut shot = None;
        for cmd in self.debug.iter().flat_map(|rx| rx.try_iter()).collect::<Vec<_>>() {
            match cmd {
                debug::Command::Press(b, secs) => self.injector.press(b, secs),
                debug::Command::Touch(keys) => self.injector.touch(keys),
                debug::Command::Scene(name) if name == "dice" => self.game.open_dice(),
                debug::Command::Scene(name) => eprintln!("debug: unknown scene {name}"),
                debug::Command::Shot(path) => shot = Some(path),
            }
        }
        let mut raw = self.raw_input();
        let (held, touch) = self.injector.frame(dt);
        raw.held |= held;
        if let Some(t) = touch {
            raw.touch = t;
            raw.touches[0] = t;
        }
        let input = self.tracker.frame(raw);
        self.home_held = if input.held.contains(Buttons::HOME) { self.home_held + dt } else { 0.0 };
        if self.home_held >= QUIT_HOLD {
            return el.exit();
        }
        #[cfg(target_os = "linux")]
        if input.pressed.contains(Buttons::POWER)
            && self.woke.elapsed() > WAKE_GRACE
            && let Some(dev) = &mut self.device
        {
            dev.suspend();
            self.last = Instant::now();
            self.woke = self.last;
            return;
        }
        self.game.update(&input, dt);
        let sounds = self.game.take_sounds();
        if let Some(audio) = &self.audio {
            audio.set_volume(self.game.volume());
            sounds.into_iter().for_each(|s| audio.play(s));
        }
        #[cfg(target_os = "linux")]
        if let Some(dev) = &mut self.device {
            dev.set_rumble(self.game.rumble());
        }
        if let Some(gfx) = &mut self.gfx {
            match gfx.render(&self.game.draw(), shot.is_some()) {
                Ok(Some(img)) => {
                    let path = shot.unwrap();
                    match debug::save_png(&path, img.width, img.height, &img.rgba) {
                        Ok(()) => eprintln!("debug: saved {}", path.display()),
                        Err(e) => eprintln!("debug: shot failed: {e}"),
                    }
                }
                Ok(None) => {}
                Err(e) => {
                    eprintln!("render: {e}");
                    el.exit();
                }
            }
        }
    }

    fn is_bottom(&self, id: WindowId) -> bool {
        self.gfx.as_ref().is_some_and(|g| g.bottom.window.id() == id)
    }

    /// Converts window pixels to screen pixels; both screens are 1024x768 regardless of window size.
    fn to_screen(&self, id: WindowId, p: PhysicalPosition<f64>) -> [f32; 2] {
        let size = self.gfx.as_ref().map_or(PhysicalSize::new(1, 1), |g| {
            if g.top.window.id() == id { g.top.window.inner_size() } else { g.bottom.window.inner_size() }
        });
        [p.x as f32 / size.width.max(1) as f32 * SCREEN_W, p.y as f32 / size.height.max(1) as f32 * SCREEN_H]
    }
}

fn key_button(code: KeyCode) -> Option<Buttons> {
    Some(match code {
        KeyCode::ArrowUp => Buttons::UP,
        KeyCode::ArrowDown => Buttons::DOWN,
        KeyCode::ArrowLeft => Buttons::LEFT,
        KeyCode::ArrowRight => Buttons::RIGHT,
        KeyCode::KeyX => Buttons::A,
        KeyCode::KeyZ => Buttons::B,
        KeyCode::KeyS => Buttons::X,
        KeyCode::KeyA => Buttons::Y,
        KeyCode::KeyQ => Buttons::L1,
        KeyCode::KeyW => Buttons::R1,
        KeyCode::Enter => Buttons::START,
        KeyCode::Backspace => Buttons::SELECT,
        KeyCode::Tab => Buttons::BACK,
        KeyCode::Digit1 => Buttons::L2,
        KeyCode::Digit2 => Buttons::R2,
        KeyCode::KeyH => Buttons::HOME,
        KeyCode::Minus => Buttons::VOL_DOWN,
        KeyCode::Equal => Buttons::VOL_UP,
        _ => return None,
    })
}

impl ApplicationHandler for App {
    fn resumed(&mut self, el: &ActiveEventLoop) {
        if self.gfx.is_some() {
            return;
        }
        match gfx::Gfx::new(el) {
            Ok(g) => self.gfx = Some(g),
            Err(e) => {
                eprintln!("gfx: {e}");
                el.exit();
            }
        }
    }

    fn window_event(&mut self, el: &ActiveEventLoop, id: WindowId, event: WindowEvent) {
        match event {
            WindowEvent::CloseRequested => el.exit(),
            WindowEvent::Resized(size) => {
                if let Some(g) = &self.gfx {
                    g.resize(id, size);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let PhysicalKey::Code(code) = event.physical_key else { return };
                let down = event.state == ElementState::Pressed;
                match code {
                    KeyCode::Escape => el.exit(),
                    KeyCode::Space => self.blow_key = down,
                    KeyCode::KeyC if down && !event.repeat => self.lid_key = !self.lid_key,
                    _ => {
                        if let Some(b) = key_button(code) {
                            self.keys.set(b, down);
                        }
                    }
                }
            }
            WindowEvent::CursorMoved { position, .. } if self.is_bottom(id) => self.cursor = Some(self.to_screen(id, position)),
            WindowEvent::CursorLeft { .. } if self.is_bottom(id) => self.cursor = None,
            WindowEvent::MouseInput { state, button: MouseButton::Left, .. } => self.mouse_down = state == ElementState::Pressed,
            // There is one touch panel, so a touch on either window is a bottom-screen touch.
            WindowEvent::Touch(t) => {
                let p = self.to_screen(id, t.location);
                let finger = self.touches.iter().position(|&(i, _)| i == t.id);
                match (t.phase, finger) {
                    (TouchPhase::Started, None) if self.touches.len() < MAX_TOUCHES => self.touches.push((t.id, p)),
                    (TouchPhase::Moved, Some(f)) => self.touches[f].1 = p,
                    (TouchPhase::Ended | TouchPhase::Cancelled, Some(f)) => {
                        self.touches.remove(f);
                    }
                    _ => {}
                }
            }
            WindowEvent::RedrawRequested if self.gfx.as_ref().is_some_and(|g| g.top.window.id() == id) => self.tick(el),
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        if let Some(g) = &self.gfx {
            g.top.window.request_redraw();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut App::new())?;
    Ok(())
}
