mod dice;
mod draw;
mod font;
mod input;
mod micro;
mod osd;
mod particles;
mod rng;
pub mod scene3d;
mod sfx;

pub use draw::{Align, Canvas, Color, Frame, Shape, Sprite};
pub use input::{Buttons, Input, InputTracker, MAX_TOUCHES, RawInput};
pub use sfx::Sfx;

use micro::{Microgame, Status};
use rng::Rng;

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
const VOLUME_MAX: u8 = 10;
const VOLUME_SHOWN: f32 = 1.2;
const SPEAKER_SHOWN: f32 = 0.6;
const WHITE: Color = [1.0, 1.0, 1.0, 1.0];

enum State {
    Title,
    Intro { t: f32, game: Box<dyn Microgame> },
    Play { t: f32, game: Box<dyn Microgame> },
    Outro { t: f32, won: bool },
    GameOver { t: f32 },
    Dice(Box<dice::Dice>),
}

pub struct Game {
    state: State,
    rng: Rng,
    score: u32,
    lives: u32,
    last_kind: usize,
    paused: bool,
    input: Input,
    rumble: f32,
    sounds: Vec<Sfx>,
    volume: u8,
    volume_shown: f32,
    /// Which speaker the stereo test last played, and for how much longer to label it.
    speaker: Option<(Sfx, f32)>,
}

impl Game {
    pub fn new(seed: u64) -> Self {
        Self {
            state: State::Title,
            rng: Rng::new(seed),
            score: 0,
            lives: LIVES,
            last_kind: usize::MAX,
            paused: false,
            input: Input::default(),
            rumble: 0.0,
            sounds: Vec::new(),
            volume: 6,
            volume_shown: 0.0,
            speaker: None,
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

    /// Jumps straight to the 3D dice scene.
    pub fn open_dice(&mut self) {
        self.state = self.dice();
    }

    fn dice(&mut self) -> State {
        State::Dice(Box::new(dice::Dice::new(&mut self.rng)))
    }

    /// Sounds queued since the last call.
    pub fn take_sounds(&mut self) -> Vec<Sfx> {
        std::mem::take(&mut self.sounds)
    }

    /// Master volume in 0..=1, set with the volume keys.
    pub fn volume(&self) -> f32 {
        self.volume as f32 / VOLUME_MAX as f32
    }

    fn outro(&mut self, won: bool) -> State {
        self.rumble = if won { RUMBLE_NICE } else { RUMBLE_MISS };
        self.sounds.push(if won { Sfx::Nice } else { Sfx::Miss });
        State::Outro { t: 0.0, won }
    }

    pub fn update(&mut self, input: &Input, dt: f32) {
        self.paused = input.lid_closed;
        self.input = *input;
        self.rumble = (self.rumble - dt).max(0.0);
        self.volume_shown = (self.volume_shown - dt).max(0.0);
        self.speaker = self.speaker.map(|(s, t)| (s, t - dt)).filter(|&(_, t)| t > 0.0);
        let step = input.pressed.contains(Buttons::VOL_UP) as i8 - input.pressed.contains(Buttons::VOL_DOWN) as i8;
        if step != 0 {
            self.volume = self.volume.saturating_add_signed(step).min(VOLUME_MAX);
            self.volume_shown = VOLUME_SHOWN;
            self.sounds.push(Sfx::Tick(self.volume));
        }
        if self.paused {
            self.rumble = 0.0;
            return;
        }
        if input.pressed.any(Buttons::BACK) {
            self.state = State::Title;
            return;
        }
        let start = input.pressed.any(Buttons::START | Buttons::A) || input.touch_started;
        self.state = match std::mem::replace(&mut self.state, State::Title) {
            State::Title if input.pressed.any(Buttons::Y) => self.dice(),
            State::Title if start => {
                self.sounds.push(Sfx::Start);
                self.score = 0;
                self.lives = LIVES;
                self.next_game()
            }
            State::Title => {
                let fresh = input.pressed & !(Buttons::VOL_UP | Buttons::VOL_DOWN | Buttons::POWER);
                let side = if fresh.any(Buttons::L1) {
                    Some(Sfx::Left)
                } else if fresh.any(Buttons::R1) {
                    Some(Sfx::Right)
                } else {
                    None
                };
                if let Some(s) = side {
                    self.sounds.push(s);
                    self.speaker = Some((s, SPEAKER_SHOWN));
                } else if fresh.0 != 0 {
                    self.sounds.push(Sfx::Tick(fresh.0.trailing_zeros() as u8));
                }
                if fresh.any(Buttons::R2) {
                    self.rumble = RUMBLE_MISS;
                }
                State::Title
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
                    State::GameOver { t: 0.0 }
                } else {
                    self.next_game()
                }
            }
            State::Outro { t, won } => State::Outro { t: t + dt, won },
            State::GameOver { t } if t > 1.0 && start => State::Title,
            State::GameOver { t } => State::GameOver { t: t + dt },
            State::Dice(mut dice) => {
                if dice.update(input, dt, &mut self.sounds).rumble {
                    self.rumble = self.rumble.max(RUMBLE_CLACK);
                }
                State::Dice(dice)
            }
        };
    }

    pub fn draw(&self) -> Frame {
        let mut f = Frame::default();
        f.top.clear = [0.08, 0.08, 0.1, 1.0];
        f.bottom.clear = [0.12, 0.12, 0.15, 1.0];
        match &self.state {
            State::Title => {
                f.top.text_centered(280.0, 32.0, WHITE, "TWINBIT");
                f.bottom.text_centered(340.0, 12.0, WHITE, "TAP OR PRESS START");
                f.bottom.text_centered(440.0, 6.0, [1.0, 1.0, 1.0, 0.5], "Y: 3D DICE   L1 R1: SPEAKERS");
                f.bottom.text_centered(490.0, 6.0, [1.0, 1.0, 1.0, 0.5], "R2: RUMBLE   HOLD HOME: QUIT");
                if let Some((side, _)) = self.speaker {
                    let label = if side == Sfx::Left { "LEFT" } else { "RIGHT" };
                    let x = if side == Sfx::Left { 60.0 } else { SCREEN_W - 60.0 - font::width(label) * 12.0 };
                    f.bottom.text(x, 600.0, 12.0, [1.0, 0.85, 0.2, 1.0], label);
                }
            }
            State::Intro { game, .. } => {
                f.top.text_centered(300.0, 36.0, [1.0, 0.85, 0.2, 1.0], game.prompt());
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
            State::GameOver { .. } => {
                f.top.text_centered(280.0, 24.0, WHITE, "GAME OVER");
                self.hud(&mut f.bottom);
            }
            State::Dice(dice) => dice.draw(&mut f),
        }
        if self.paused {
            f.top.text_centered(360.0, 16.0, WHITE, "PAUSED");
        }
        if self.volume_shown > 0.0 {
            f.top.text_centered(40.0, 8.0, WHITE, &format!("VOL {}", self.volume));
        }
        osd::draw(&mut f.top, &self.input, self.rumble());
        osd::touches(&mut f.bottom, &self.input);
        f
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

    #[test]
    fn idle_play_ends_in_game_over() {
        let mut game = Game::new(1);
        let mut t = InputTracker::default();
        game.update(&t.frame(RawInput { held: Buttons::START, ..Default::default() }), 0.016);
        for _ in 0..10_000 {
            game.update(&t.frame(RawInput::default()), 0.016);
        }
        assert!(matches!(game.state, State::GameOver { .. }));
        assert_eq!(game.lives, 0);
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
    fn volume_keys_clamp_and_beep() {
        let mut game = Game::new(1);
        for _ in 0..20 {
            game.update(&Input { pressed: Buttons::VOL_UP, ..Default::default() }, 0.016);
        }
        assert_eq!(game.volume(), 1.0);
        assert!(game.take_sounds().iter().all(|s| matches!(s, Sfx::Tick(_))));
    }

    #[test]
    fn y_on_the_title_opens_the_dice_and_back_returns() {
        let mut game = Game::new(1);
        game.update(&Input { pressed: Buttons::Y, ..Default::default() }, 0.016);
        assert!(matches!(game.state, State::Dice(_)));
        game.update(&Input { pressed: Buttons::BACK, ..Default::default() }, 0.016);
        assert!(matches!(game.state, State::Title));
    }

    #[test]
    fn closing_the_lid_freezes_the_game() {
        let mut game = Game::new(1);
        let input = Input { lid_closed: true, pressed: Buttons::START, ..Default::default() };
        game.update(&input, 0.016);
        assert!(matches!(game.state, State::Title));
    }
}
