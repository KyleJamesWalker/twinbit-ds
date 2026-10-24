use std::error::Error;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, SizedSample};
use twinbit_game::Sfx;

const MIX: f32 = 0.25;
const ATTACK: f32 = 0.005;

#[derive(Clone, Copy)]
enum Wave {
    Square,
    Triangle,
    Noise,
}

/// One synth voice: a pitch sweep from `f0` to `f1` over `dur` seconds, starting after `delay`.
/// `pan` runs from -1 (left speaker only) to 1 (right speaker only).
#[derive(Clone, Copy)]
struct Note {
    delay: f32,
    dur: f32,
    f0: f32,
    f1: f32,
    wave: Wave,
    gain: f32,
    pan: f32,
}

const fn note(delay: f32, dur: f32, f0: f32, f1: f32, wave: Wave, gain: f32) -> Note {
    Note { delay, dur, f0, f1, wave, gain, pan: 0.0 }
}

fn arpeggio(freqs: &[f32], step: f32, wave: Wave) -> Vec<Note> {
    freqs.iter().enumerate().map(|(i, &f)| note(i as f32 * step, step * 1.4, f, f, wave, 0.8)).collect()
}

fn panned(notes: Vec<Note>, pan: f32) -> Vec<Note> {
    notes.into_iter().map(|n| Note { pan, ..n }).collect()
}

fn notes(sfx: Sfx) -> Vec<Note> {
    use Wave::*;
    match sfx {
        Sfx::Tick(n) => {
            let f = 440.0 * 2f32.powf(n as f32 / 12.0);
            vec![note(0.0, 0.06, f, f, Square, 0.6)]
        }
        Sfx::Left => panned(arpeggio(&[523.0, 659.0, 523.0], 0.09, Square), -1.0),
        Sfx::Right => panned(arpeggio(&[784.0, 988.0, 784.0], 0.09, Square), 1.0),
        Sfx::Start => arpeggio(&[523.0, 659.0, 784.0, 1047.0], 0.07, Square),
        Sfx::Prompt => arpeggio(&[784.0, 1047.0], 0.06, Square),
        Sfx::Catch => vec![note(0.0, 0.09, 880.0, 1760.0, Square, 0.7)],
        Sfx::Pop => vec![note(0.0, 0.18, 1.0, 1.0, Noise, 1.0), note(0.0, 0.15, 700.0, 120.0, Triangle, 1.0)],
        Sfx::Hit => vec![note(0.0, 0.3, 1.0, 1.0, Noise, 0.9), note(0.0, 0.3, 220.0, 55.0, Square, 0.7)],
        Sfx::Nice => arpeggio(&[659.0, 784.0, 1047.0], 0.08, Square),
        Sfx::Miss => vec![note(0.0, 0.35, 392.0, 147.0, Square, 0.8)],
        Sfx::Clack(force) => {
            let g = (force as f32 / 60.0).clamp(0.25, 1.0);
            vec![note(0.0, 0.05, 1.0, 1.0, Noise, g), note(0.0, 0.06, 900.0, 500.0, Triangle, g * 0.6)]
        }
        Sfx::GameOver => arpeggio(&[523.0, 392.0, 330.0, 262.0], 0.16, Triangle),
    }
}

struct Voice {
    note: Note,
    t: f32,
    phase: f32,
}

struct Synth {
    rx: Receiver<Sfx>,
    voices: Vec<Voice>,
    volume: Arc<AtomicU32>,
    dt: f32,
    noise: u32,
}

impl Synth {
    fn sample(&mut self) -> [f32; 2] {
        let mut sum = [0.0; 2];
        for v in &mut self.voices {
            v.t += self.dt;
            if v.t < 0.0 {
                continue;
            }
            let n = v.note;
            let freq = n.f0 * (n.f1 / n.f0).powf(v.t / n.dur);
            v.phase = (v.phase + freq * self.dt).fract();
            let wave = match n.wave {
                Wave::Square => if v.phase < 0.5 { 1.0 } else { -1.0 },
                Wave::Triangle => 4.0 * (v.phase - 0.5).abs() - 1.0,
                Wave::Noise => {
                    self.noise ^= self.noise << 13;
                    self.noise ^= self.noise >> 17;
                    self.noise ^= self.noise << 5;
                    self.noise as f32 / u32::MAX as f32 * 2.0 - 1.0
                }
            };
            let env = (v.t / ATTACK).min(1.0) * (1.0 - v.t / n.dur).max(0.0);
            let s = wave * env * n.gain;
            sum[0] += s * (1.0 - n.pan).min(1.0);
            sum[1] += s * (1.0 + n.pan).min(1.0);
        }
        self.voices.retain(|v| v.t < v.note.dur);
        let gain = MIX * f32::from_bits(self.volume.load(Ordering::Relaxed));
        sum.map(|s| (s * gain).clamp(-1.0, 1.0))
    }

    fn fill<T: SizedSample + cpal::FromSample<f32>>(&mut self, data: &mut [T], channels: usize) {
        while let Ok(sfx) = self.rx.try_recv() {
            self.voices.extend(notes(sfx).into_iter().map(|note| Voice { note, t: -note.delay, phase: 0.0 }));
        }
        for frame in data.chunks_mut(channels) {
            let [l, r] = self.sample();
            match frame {
                [left, right, rest @ ..] => {
                    *left = T::from_sample(l);
                    *right = T::from_sample(r);
                    rest.fill(T::from_sample((l + r) / 2.0));
                }
                _ => frame.fill(T::from_sample((l + r) / 2.0)),
            }
        }
    }
}

pub struct Audio {
    _stream: cpal::Stream,
    tx: Sender<Sfx>,
    volume: Arc<AtomicU32>,
}

impl Audio {
    pub fn open() -> Result<Self, Box<dyn Error>> {
        let device = cpal::default_host().default_output_device().ok_or("no output device")?;
        let supported = device.default_output_config()?;
        eprintln!("audio: {:?} {:?}", device.name(), supported);
        let config = supported.config();
        let channels = config.channels as usize;
        let (tx, rx) = channel();
        let volume = Arc::new(AtomicU32::new(0f32.to_bits()));
        let mut synth = Synth { rx, voices: Vec::new(), volume: volume.clone(), dt: 1.0 / config.sample_rate.0 as f32, noise: 0x9E37_79B9 };
        let stream = match supported.sample_format() {
            SampleFormat::F32 => device.build_output_stream(&config, move |d: &mut [f32], _: &_| synth.fill(d, channels), on_error, None)?,
            SampleFormat::I16 => device.build_output_stream(&config, move |d: &mut [i16], _: &_| synth.fill(d, channels), on_error, None)?,
            f => return Err(format!("unsupported output format {f:?}").into()),
        };
        stream.play()?;
        Ok(Self { _stream: stream, tx, volume })
    }

    pub fn play(&self, sfx: Sfx) {
        self.tx.send(sfx).ok();
    }

    pub fn set_volume(&self, v: f32) {
        self.volume.store(v.to_bits(), Ordering::Relaxed);
    }
}

fn on_error(e: cpal::StreamError) {
    eprintln!("audio: {e}");
}
