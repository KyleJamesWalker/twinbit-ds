/// Sound cues the game asks the backend to play; the backend owns the synth.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Sfx {
    /// Button tester blip; the value picks the pitch.
    Tick(u8),
    /// Stereo test tones, one per speaker.
    Left,
    Right,
    Start,
    Prompt,
    Catch,
    Pop,
    Hit,
    Nice,
    Miss,
    GameOver,
    /// Die hitting the tray; louder for a harder hit.
    Clack(u8),
}
