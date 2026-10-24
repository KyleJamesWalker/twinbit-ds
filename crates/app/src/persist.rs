//! Save file and crash log, kept next to the binary unless `TWINBIT_SAVE` names another save path.

use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use twinbit_game::Save;

/// Batches bursts of changes, such as holding a volume key, into one write.
const SETTLE: Duration = Duration::from_secs(1);

fn dir() -> PathBuf {
    std::env::current_exe().ok().and_then(|p| p.parent().map(Path::to_path_buf)).unwrap_or_else(|| PathBuf::from("."))
}

pub struct Saver {
    path: PathBuf,
    written: Save,
    changed_at: Option<Instant>,
}

impl Saver {
    pub fn load() -> (Self, Save) {
        let path = std::env::var_os("TWINBIT_SAVE").map_or_else(|| dir().join("save.txt"), PathBuf::from);
        let save = fs::read_to_string(&path).map(|t| Save::parse(&t)).unwrap_or_default();
        eprintln!("save: {} {save:?}", path.display());
        (Self { path, written: save, changed_at: None }, save)
    }

    /// Writes `save` once it has stopped changing, or right away with `now`.
    pub fn update(&mut self, save: Save, now: bool) {
        if save == self.written {
            self.changed_at = None;
            return;
        }
        let since = *self.changed_at.get_or_insert_with(Instant::now);
        if now || since.elapsed() >= SETTLE {
            match write_atomic(&self.path, save.to_text().as_bytes()) {
                Ok(()) => self.written = save,
                Err(e) => eprintln!("save: {}: {e}", self.path.display()),
            }
            self.changed_at = None;
        }
    }
}

/// The SD card is vfat and may lose power at any moment, so write a temp file, sync it, then rename.
fn write_atomic(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp");
    let mut f = File::create(&tmp)?;
    f.write_all(bytes)?;
    f.sync_all()?;
    fs::rename(&tmp, path)?;
    if let Some(parent) = path.parent() {
        File::open(parent).and_then(|d| d.sync_all()).ok();
    }
    Ok(())
}

/// Appends each panic, with a backtrace, to `crash.log` before the default hook prints it.
pub fn install_crash_log() {
    let log = dir().join("crash.log");
    let default = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let when = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs());
        let report = format!("--- crash at unix {when}\n{info}\n{}\n", std::backtrace::Backtrace::force_capture());
        if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&log) {
            f.write_all(report.as_bytes()).ok();
            f.sync_all().ok();
        }
        default(info);
    }));
}
