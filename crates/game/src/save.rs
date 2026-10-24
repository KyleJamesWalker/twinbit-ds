//! Settings and records kept between runs, as `key=value` lines.

pub const LEVELS: u8 = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Save {
    pub volume: u8,
    /// Per-screen brightness in 1..=LEVELS, top first.
    pub brightness: [u8; 2],
    pub best: u32,
}

impl Default for Save {
    fn default() -> Self {
        Self { volume: 6, brightness: [4, 4], best: 0 }
    }
}

impl Save {
    pub fn to_text(&self) -> String {
        format!(
            "volume={}\nbrightness_top={}\nbrightness_bottom={}\nbest={}\n",
            self.volume, self.brightness[0], self.brightness[1], self.best
        )
    }

    /// Unknown keys and bad values fall back to the defaults, so an old or damaged file still loads.
    pub fn parse(text: &str) -> Self {
        let mut save = Self::default();
        for (key, value) in text.lines().filter_map(|l| l.split_once('=')) {
            let value = value.trim();
            match key.trim() {
                "volume" => save.volume = value.parse().unwrap_or(save.volume).min(LEVELS),
                "brightness_top" => save.brightness[0] = value.parse().unwrap_or(save.brightness[0]).clamp(1, LEVELS),
                "brightness_bottom" => save.brightness[1] = value.parse().unwrap_or(save.brightness[1]).clamp(1, LEVELS),
                "best" => save.best = value.parse().unwrap_or(save.best),
                _ => {}
            }
        }
        save
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let save = Save { volume: 3, brightness: [7, 2], best: 41 };
        assert_eq!(Save::parse(&save.to_text()), save);
    }

    #[test]
    fn damaged_files_fall_back_per_field() {
        let save = Save::parse("volume=99\nbrightness_top=0\nbest=oops\njunk\nbrightness_bottom=8");
        assert_eq!(save, Save { volume: LEVELS, brightness: [1, 8], best: 0 });
    }
}
