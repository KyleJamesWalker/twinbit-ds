mod dice;
mod draw;
mod font;
mod hardware;
mod input;
mod micro;
mod osd;
mod particles;
mod rng;
mod save;
pub mod scene3d;
mod sfx;
mod stress;
mod system;

pub use draw::{Align, Canvas, Color, Frame, Shape, Sprite};
pub use input::{Buttons, Input, InputTracker, MAX_TOUCHES, RawInput};
pub use save::Save;
pub use sfx::Sfx;
pub use system::{Battery, Outputs, System};

use micro::{Microgame, Status};
use rng::Rng;
use save::LEVELS;

pub const SCREEN_W: f32 = 1024.0;
pub const SCREEN_H: f32 = 768.0;
/// Virtual distance across the hinge, so objects take a moment to cross between screens.
pub const HINGE_GAP: f32 = 96.0;
pub const BOTTOM_OFFSET: f32 = SCREEN_H + HINGE_GAP;
pub const WORLD_H: f32 = BOTTOM_OFFSET + SCREEN_H;

const LIVES: u32 = 3;
const INTRO: f32 = 0.9;
const OUTRO: f32 = 0.9;
const RUMBLE_MISS: f32 = 0.35;
const RUMBLE_NICE: f32 = 0.12;
const RUMBLE_CLACK: f32 = 0.05;
const VOLUME_SHOWN: f32 = 1.2;
const WHITE: Color = [1.0, 1.0, 1.0, 1.0];
const HOT: Color = [1.0, 0.85, 0.2, 1.0];
const MENU_TOP: f32 = 170.0;
const MENU_ROW: f32 = 110.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Scene {
    Microgames,
    /// A microgame run that starts with one microgame, by index.
    Microgame(usize),
    Dice,
    Stress,
    Hardware,
}

impl Scene {
    const MENU: [(Scene, &'static str); 4] =
        [(Scene::Microgames, "Microgames"), (Scene::Dice, "3D dice"), (Scene::Stress, "Stress test"), (Scene::Hardware, "Hardware test")];

    pub fn from_name(name: &str) -> Option<Scene> {
        Some(match name {
            "micro" | "microgames" => Scene::Microgames,
            "stars" => Scene::Microgame(0),
            "balloon" => Scene::Microgame(1),
            "dodge" => Scene::Microgame(2),
            "dice" => Scene::Dice,
            "stress" => Scene::Stress,
            "hardware" => Scene::Hardware,
            _ => return None,
        })
    }
}

enum State {
    Title { selected: usize },
    Intro { t: f32, game: Box<dyn Microgame> },
    Play { t: f32, game: Box<dyn Microgame> },
    Outro { t: f32, won: bool },
    GameOver { t: f32, best: bool },
    Dice(Box<dice::Dice>),
    Stress(Box<stress::Stress>),
    Hardware(hardware::Hardware),
}

pub struct Game {
    state: State,
    rng: Rng,
    score: u32,
    lives: u32,
    last_kind: usize,
    paused: bool,
    input: Input,
    system: System,
    show_perf: bool,
    rumble: f32,
    sounds: Vec<Sfx>,
    save: Save,
    volume_shown: f32,
    leds: [bool; 2],
}

impl Game {
    pub fn new(seed: u64) -> Self {
        Self {
            state: State::Title { selected: 0 },
            rng: Rng::new(seed),
            score: 0,
            lives: LIVES,
            last_kind: usize::MAX,
            paused: false,
            input: Input::default(),
            system: System::default(),
            show_perf: false,
            rumble: 0.0,
            sounds: Vec::new(),
            save: Save::default(),
            volume_shown: 0.0,
            leds: [false; 2],
        }
    }

    pub fn load(&mut self, save: Save) {
        self.save = save;
    }

    pub fn save(&self) -> Save {
        self.save
    }

    /// One line describing the current scene, for logs.
    pub fn summary(&self) -> String {
        match &self.state {
            State::Title { .. } => "menu".into(),
            State::Intro { .. } | State::Play { .. } | State::Outro { .. } | State::GameOver { .. } => format!("microgames score={}", self.score),
            State::Dice(_) => "dice".into(),
            State::Stress(s) => {
                let [top, bottom] = s.counts();
                format!("stress top={top} bottom={bottom}")
            }
            State::Hardware(_) => "hardware".into(),
        }
    }

    pub fn set_system(&mut self, system: System) {
        self.system = system;
    }

    pub fn open(&mut self, scene: Scene) {
        self.state = self.enter(scene);
    }

    fn enter(&mut self, scene: Scene) -> State {
        match scene {
            Scene::Microgames => {
                self.sounds.push(Sfx::Start);
                self.score = 0;
                self.lives = LIVES;
                self.next_game()
            }
            Scene::Microgame(kind) => {
                self.score = 0;
                self.lives = LIVES;
                self.last_kind = kind;
                let speed = self.speed();
                State::Intro { t: 0.0, game: micro::make(kind, &mut self.rng, speed) }
            }
            Scene::Dice => State::Dice(Box::new(dice::Dice::new(&mut self.rng))),
            Scene::Stress => State::Stress(Box::new(stress::Stress::new(&mut self.rng))),
            Scene::Hardware => State::Hardware(hardware::Hardware::default()),
        }
    }

    fn speed(&self) -> f32 {
        (1.0 + self.score as f32 * 0.08).min(2.0)
    }

    fn next_game(&mut self) -> State {
        let mut kind = self.rng.below(micro::KINDS);
        if kind == self.last_kind {
            kind = (kind + 1) % micro::KINDS;
        }
        self.last_kind = kind;
        self.sounds.push(Sfx::Prompt);
        let speed = self.speed();
        State::Intro { t: 0.0, game: micro::make(kind, &mut self.rng, speed) }
    }

    /// Whether the vibration motor should be running this frame.
    pub fn rumble(&self) -> bool {
        self.rumble > 0.0
    }

    pub fn outputs(&self) -> Outputs {
        let level = |l: u8| l as f32 / LEVELS as f32;
        Outputs {
            rumble: self.rumble(),
            volume: self.volume(),
            brightness: self.save.brightness.map(level),
            screens_on: !self.paused,
            work_led: self.leds[0],
            low_led: self.leds[1],
        }
    }

    /// Drains the queued sound cues.
    pub fn take_sounds(&mut self) -> Vec<Sfx> {
        std::mem::take(&mut self.sounds)
    }

    /// Master volume in 0..=1, set with the volume keys.
    pub fn volume(&self) -> f32 {
        self.save.volume as f32 / LEVELS as f32
    }

    fn outro(&mut self, won: bool) -> State {
        self.rumble = if won { RUMBLE_NICE } else { RUMBLE_MISS };
        self.sounds.push(if won { Sfx::Nice } else { Sfx::Miss });
        State::Outro { t: 0.0, won }
    }

    fn menu_hit(&self, input: &Input) -> Option<usize> {
        let [_, y] = input.touch.filter(|_| input.touch_started)?;
        let row = ((y - MENU_TOP + MENU_ROW / 2.0) / MENU_ROW).floor();
        (row >= 0.0 && (row as usize) < Scene::MENU.len()).then_some(row as usize)
    }

    pub fn update(&mut self, input: &Input, dt: f32) {
        self.paused = input.lid_closed;
        self.input = *input;
        self.rumble = (self.rumble - dt).max(0.0);
        self.volume_shown = (self.volume_shown - dt).max(0.0);
        let step = input.pressed.contains(Buttons::VOL_UP) as i8 - input.pressed.contains(Buttons::VOL_DOWN) as i8;
        if step != 0 {
            self.save.volume = self.save.volume.saturating_add_signed(step).min(LEVELS);
            self.volume_shown = VOLUME_SHOWN;
            self.sounds.push(Sfx::Tick(self.save.volume));
        }
        if input.pressed.contains(Buttons::SELECT) && !matches!(self.state, State::Hardware(_)) {
            self.show_perf = !self.show_perf;
        }
        if self.paused {
            self.rumble = 0.0;
            return;
        }
        if input.pressed.any(Buttons::BACK) {
            self.state = State::Title { selected: 0 };
            return;
        }
        let start = input.pressed.any(Buttons::START | Buttons::A) || input.touch_started;
        self.state = match std::mem::replace(&mut self.state, State::Title { selected: 0 }) {
            State::Title { selected } => {
                let n = Scene::MENU.len();
                let moved = if input.pressed.contains(Buttons::DOWN) {
                    (selected + 1) % n
                } else if input.pressed.contains(Buttons::UP) {
                    (selected + n - 1) % n
                } else {
                    selected
                };
                if moved != selected {
                    self.sounds.push(Sfx::Tick(moved as u8 * 3));
                }
                let chosen = self.menu_hit(input).or((input.pressed.any(Buttons::START | Buttons::A)).then_some(moved));
                match chosen {
                    Some(i) => self.enter(Scene::MENU[i].0),
                    None => State::Title { selected: moved },
                }
            }
            State::Intro { t, game } if t + dt >= INTRO => State::Play { t: 0.0, game },
            State::Intro { t, game } => State::Intro { t: t + dt, game },
            State::Play { t, mut game } => {
                let t = t + dt;
                match game.update(input, dt, &mut self.sounds) {
                    Status::Won => self.outro(true),
                    Status::Lost => self.outro(false),
                    Status::Running if t >= game.duration() => self.outro(game.win_on_timeout()),
                    Status::Running => State::Play { t, game },
                }
            }
            State::Outro { t, won } if t + dt >= OUTRO => {
                if won {
                    self.score += 1;
                } else {
                    self.lives -= 1;
                }
                if self.lives == 0 {
                    self.sounds.push(Sfx::GameOver);
                    let best = self.score > self.save.best;
                    self.save.best = self.save.best.max(self.score);
                    State::GameOver { t: 0.0, best }
                } else {
                    self.next_game()
                }
            }
            State::Outro { t, won } => State::Outro { t: t + dt, won },
            State::GameOver { t, .. } if t > 1.0 && start => State::Title { selected: 0 },
            State::GameOver { t, best } => State::GameOver { t: t + dt, best },
            State::Dice(mut dice) => {
                if dice.update(input, dt, &mut self.sounds).rumble {
                    self.rumble = self.rumble.max(RUMBLE_CLACK);
                }
                State::Dice(dice)
            }
            State::Stress(mut stress) => {
                stress.update(input, dt, &mut self.sounds);
                State::Stress(stress)
            }
            State::Hardware(mut hw) => {
                let controls = hardware::Controls {
                    sounds: &mut self.sounds,
                    rumble: &mut self.rumble,
                    brightness: &mut self.save.brightness,
                    leds: &mut self.leds,
                };
                hw.update(input, dt, controls);
                State::Hardware(hw)
            }
        };
    }

    pub fn draw(&self) -> Frame {
        let mut f = Frame::default();
        f.top.clear = [0.08, 0.08, 0.1, 1.0];
        f.bottom.clear = [0.12, 0.12, 0.15, 1.0];
        let mut perf = self.show_perf;
        match &self.state {
            State::Title { selected } => self.draw_menu(&mut f, *selected),
            State::Intro { game, .. } => {
                f.top.text_centered(300.0, 36.0, HOT, game.prompt());
                self.hud(&mut f.bottom);
            }
            State::Play { t, game } => {
                game.draw(&mut f);
                let left = 1.0 - t / game.duration();
                f.top.rect(0.0, 0.0, SCREEN_W * left, 16.0, [1.0, 1.0, 1.0, 0.8]);
            }
            State::Outro { won, .. } => {
                let (msg, color) = if *won { ("NICE!", [0.3, 1.0, 0.4, 1.0]) } else { ("MISS!", [1.0, 0.3, 0.3, 1.0]) };
                f.top.text_centered(300.0, 36.0, color, msg);
                self.hud(&mut f.bottom);
            }
            State::GameOver { best, .. } => {
                f.top.text_centered(240.0, 24.0, WHITE, "GAME OVER");
                if *best {
                    f.top.label("New best!", SCREEN_W / 2.0, 400.0, 56.0, HOT, Align::Center);
                }
                self.hud(&mut f.bottom);
            }
            State::Dice(dice) => dice.draw(&mut f),
            State::Stress(stress) => {
                stress.draw(&mut f);
                perf = true;
            }
            State::Hardware(hw) => hw.draw(&mut f, &self.system, self.save.brightness, self.leds),
        }
        if self.paused {
            f.top.text_centered(360.0, 16.0, WHITE, "PAUSED");
        }
        if self.volume_shown > 0.0 {
            f.top.text_centered(40.0, 8.0, WHITE, &format!("VOL {}", self.save.volume));
        }
        if perf {
            system::draw_perf(&mut f.top, &self.system);
        } else if matches!(self.state, State::Title { .. }) {
            system::draw_battery(&mut f.top, &self.system, 16.0);
        }
        osd::draw(&mut f.top, &self.input, self.rumble(), self.system.headphones);
        osd::touches(&mut f.bottom, &self.input);
        f
    }

    fn draw_menu(&self, f: &mut Frame, selected: usize) {
        f.top.text_centered(200.0, 32.0, WHITE, "TWINBIT");
        f.top.label("ANBERNIC RG DS Plus tech demo", SCREEN_W / 2.0, 400.0, 40.0, [1.0, 1.0, 1.0, 0.7], Align::Center);
        if self.save.best > 0 {
            f.top.label(format!("Best microgame run: {}", self.save.best), SCREEN_W / 2.0, 470.0, 32.0, HOT, Align::Center);
        }
        for (i, (_, name)) in Scene::MENU.iter().enumerate() {
            let y = MENU_TOP + i as f32 * MENU_ROW;
            let on = i == selected;
            if on {
                f.bottom.rect(212.0, y - MENU_ROW / 2.0 + 8.0, 600.0, MENU_ROW - 16.0, [1.0, 0.85, 0.2, 0.25]);
            }
            f.bottom.label(*name, SCREEN_W / 2.0, y - 28.0, 52.0, if on { HOT } else { WHITE }, Align::Center);
        }
        let hint = "Tap, or Up / Down and A.   Select: performance overlay.   Hold Home: quit";
        f.bottom.label(hint, SCREEN_W / 2.0, SCREEN_H - 48.0, 22.0, [1.0, 1.0, 1.0, 0.5], Align::Center);
    }

    fn hud(&self, c: &mut Canvas) {
        c.text_centered(300.0, 16.0, WHITE, &format!("SCORE {}", self.score));
        for i in 0..LIVES {
            let color = if i < self.lives { [1.0, 0.3, 0.4, 1.0] } else { [0.3, 0.3, 0.3, 1.0] };
            c.circle(SCREEN_W / 2.0 + (i as f32 - 1.0) * 80.0, 480.0, 24.0, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn press(game: &mut Game, b: Buttons) {
        game.update(&Input { pressed: b, ..Default::default() }, 0.016);
    }

    #[test]
    fn idle_play_ends_in_game_over_and_records_the_best() {
        let mut game = Game::new(1);
        let mut t = InputTracker::default();
        game.update(&t.frame(RawInput { held: Buttons::START, ..Default::default() }), 0.016);
        for _ in 0..10_000 {
            game.update(&t.frame(RawInput::default()), 0.016);
        }
        assert!(matches!(game.state, State::GameOver { .. }));
        assert_eq!(game.lives, 0);
        assert_eq!(game.save().best, game.score);
    }

    #[test]
    fn a_miss_rumbles_then_stops() {
        let mut game = Game::new(1);
        game.state = game.outro(false);
        assert!(game.rumble());
        game.update(&Input::default(), RUMBLE_MISS + 0.01);
        assert!(!game.rumble());
    }

    #[test]
    fn volume_keys_clamp_beep_and_persist() {
        let mut game = Game::new(1);
        for _ in 0..20 {
            press(&mut game, Buttons::VOL_UP);
        }
        assert_eq!(game.volume(), 1.0);
        assert_eq!(game.save().volume, LEVELS);
        assert!(game.take_sounds().iter().all(|s| matches!(s, Sfx::Tick(_))));
    }

    #[test]
    fn the_menu_opens_each_scene_and_back_returns() {
        let mut game = Game::new(1);
        press(&mut game, Buttons::DOWN);
        press(&mut game, Buttons::A);
        assert!(matches!(game.state, State::Dice(_)));
        press(&mut game, Buttons::BACK);
        assert!(matches!(game.state, State::Title { selected: 0 }));
        press(&mut game, Buttons::UP);
        press(&mut game, Buttons::A);
        assert!(matches!(game.state, State::Hardware(_)));
    }

    #[test]
    fn tapping_a_menu_row_opens_it() {
        let mut game = Game::new(1);
        let y = MENU_TOP + 2.0 * MENU_ROW;
        game.update(&Input { touch: Some([512.0, y]), touch_started: true, ..Default::default() }, 0.016);
        assert!(matches!(game.state, State::Stress(_)));
    }

    #[test]
    fn closing_the_lid_freezes_the_game_and_turns_the_screens_off() {
        let mut game = Game::new(1);
        let input = Input { lid_closed: true, pressed: Buttons::START, ..Default::default() };
        game.update(&input, 0.016);
        assert!(matches!(game.state, State::Title { .. }));
        assert!(!game.outputs().screens_on);
    }
}
