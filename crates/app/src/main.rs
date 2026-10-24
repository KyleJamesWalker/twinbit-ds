mod audio;
mod debug;
#[cfg(target_os = "linux")]
mod device;
mod gfx;
mod mic;
mod persist;

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use twinbit_game::{Buttons, Game, InputTracker, MAX_TOUCHES, RawInput, SCREEN_H, SCREEN_W, Scene, System};
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
const STATS_WINDOW: Duration = Duration::from_secs(1);

/// Frame timing over the last window, plus sensors read once per window.
#[derive(Default)]
struct Stats {
    frames: u32,
    total_ms: f32,
    worst_ms: f32,
    since: Option<Instant>,
}

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
    saver: persist::Saver,
    stats: Stats,
    system: System,
}

impl App {
    fn new() -> Self {
        let seed = SystemTime::now().duration_since(UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64);
        let mut game = Game::new(seed);
        let (saver, save) = persist::Saver::load();
        game.load(save);
        if let Some(scene) = std::env::var("TWINBIT_START").ok().and_then(|s| Scene::from_name(&s)) {
            game.open(scene);
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
            saver,
            stats: Stats::default(),
            system: System::default(),
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

    /// Folds one frame into the stats and refreshes the system report once per window.
    fn measure(&mut self, frame: Duration) {
        let ms = frame.as_secs_f32() * 1000.0;
        let s = &mut self.stats;
        s.frames += 1;
        s.total_ms += ms;
        s.worst_ms = s.worst_ms.max(ms);
        let since = *s.since.get_or_insert_with(Instant::now);
        if since.elapsed() < STATS_WINDOW {
            return;
        }
        self.system.fps = s.frames as f32 / since.elapsed().as_secs_f32();
        self.system.frame_ms = s.total_ms / s.frames as f32;
        self.system.worst_ms = s.worst_ms;
        *s = Stats { since: Some(Instant::now()), ..Default::default() };
        if self.debug.is_some() {
            let sys = &self.system;
            eprintln!("perf: fps={:.1} ms={:.1} worst={:.1} {}", sys.fps, sys.frame_ms, sys.worst_ms, self.game.summary());
        }
        #[cfg(target_os = "linux")]
        if let Some(dev) = &self.device {
            self.system.soc_c = dev.temperature("soc-thermal");
            self.system.gpu_c = dev.temperature("gpu-thermal");
            self.system.battery = dev.battery();
        }
    }

    #[cfg(target_os = "linux")]
    fn suspend(&mut self, wake_after: Option<u32>) {
        self.saver.update(self.game.save(), true);
        if let Some(dev) = &mut self.device {
            dev.suspend(wake_after);
            eprintln!("resumed");
        }
        self.last = Instant::now();
        self.woke = self.last;
        self.stats = Stats::default();
    }

    fn tick(&mut self, el: &ActiveEventLoop) {
        let now = Instant::now();
        let frame = now - self.last;
        let dt = frame.as_secs_f32().min(0.05);
        self.last = now;
        self.measure(frame);
        let mut shot = None;
        for cmd in self.debug.iter().flat_map(|rx| rx.try_iter()).collect::<Vec<_>>() {
            match cmd {
                debug::Command::Press(b, secs) => self.injector.press(b, secs),
                debug::Command::Touch(keys) => self.injector.touch(keys),
                debug::Command::Scene(name) => match Scene::from_name(&name) {
                    Some(scene) => self.game.open(scene),
                    None => eprintln!("debug: unknown scene {name}"),
                },
                debug::Command::Shot(path) => shot = Some(path),
                #[cfg(target_os = "linux")]
                debug::Command::Sleep(secs) => return self.suspend(Some(secs)),
                #[cfg(not(target_os = "linux"))]
                debug::Command::Sleep(secs) => eprintln!("debug: sleep {secs} needs the device"),
                debug::Command::Panic => panic!("debug: panic requested"),
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
            && self.device.is_some()
        {
            return self.suspend(None);
        }
        #[cfg(target_os = "linux")]
        if let Some(dev) = &self.device {
            self.system.headphones = dev.headphones();
        }
        self.game.set_system(self.system);
        self.game.update(&input, dt);
        self.saver.update(self.game.save(), false);
        let sounds = self.game.take_sounds();
        if let Some(audio) = &self.audio {
            audio.set_volume(self.game.volume());
            sounds.into_iter().for_each(|s| audio.play(s));
        }
        #[cfg(target_os = "linux")]
        if let Some(dev) = &mut self.device {
            dev.apply(&self.game.outputs());
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

    /// Saves and restores the device, then exits without running the GL and Wayland teardown.
    fn exiting(&mut self, _el: &ActiveEventLoop) {
        self.saver.update(self.game.save(), true);
        #[cfg(target_os = "linux")]
        drop(self.device.take());
        std::process::exit(0);
    }

    fn about_to_wait(&mut self, _el: &ActiveEventLoop) {
        if let Some(g) = &self.gfx {
            g.top.window.request_redraw();
        }
    }
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    persist::install_crash_log();
    let event_loop = EventLoop::new()?;
    event_loop.run_app(&mut App::new())?;
    Ok(())
}
