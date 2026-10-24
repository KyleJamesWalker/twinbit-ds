use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};

use cpal::SampleFormat;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};

/// RMS level of the default capture device, updated from the audio thread.
pub struct Mic {
    _stream: cpal::Stream,
    level: Arc<AtomicU32>,
}

impl Mic {
    pub fn open() -> Result<Self, Box<dyn Error>> {
        let device = cpal::default_host().default_input_device().ok_or("no input device")?;
        let supported = device.default_input_config()?;
        eprintln!("mic: {:?} {:?}", device.name(), supported);
        let config = supported.config();
        let level = Arc::new(AtomicU32::new(0));
        let l = level.clone();
        let stream = match supported.sample_format() {
            SampleFormat::F32 => device.build_input_stream(&config, move |d: &[f32], _: &_| store(&l, d.iter().copied()), on_error, None)?,
            SampleFormat::I16 => device.build_input_stream(
                &config,
                move |d: &[i16], _: &_| store(&l, d.iter().map(|&s| s as f32 / 32768.0)),
                on_error,
                None,
            )?,
            f => return Err(format!("unsupported mic format {f:?}").into()),
        };
        stream.play()?;
        Ok(Self { _stream: stream, level })
    }

    pub fn level(&self) -> f32 {
        f32::from_bits(self.level.load(Ordering::Relaxed))
    }
}

fn store(level: &AtomicU32, samples: impl Iterator<Item = f32>) {
    let (sum, n) = samples.fold((0.0, 0usize), |(sum, n), s| (sum + s * s, n + 1));
    if n > 0 {
        level.store((sum / n as f32).sqrt().to_bits(), Ordering::Relaxed);
    }
}

fn on_error(e: cpal::StreamError) {
    eprintln!("mic: {e}");
}
