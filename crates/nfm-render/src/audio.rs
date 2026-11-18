//! `audio` — the cpal desktop audio backend (`AudioSink` + device setup).
//!
//! The game voices run at 7500–9000 Hz, the `.au` clips at 8000 Hz. The
//! bus resamples by fractional cursor (`AudioBus::pull` takes ANY output
//! rate — pitch stays correct regardless), so the device rate is a
//! fidelity choice, not a correctness one:
//!
//! * Prefer an 8000 Hz mono i16 config when the device offers one (zero
//!   resampling for every clip, 1:1 for most music).
//! * Else the closest rate the device allows (`clamp(8000, min, max)`),
//!   mono preferred, i16 preferred over f32; stereo (or more channels)
//!   falls back to duplicating the mono mix per channel.
//! * Else (no usable config at all, e.g. only exotic sample formats):
//!   give up and let play run silent — the same `None`-audio path as a
//!   missing device.
//!
//! Only i16 and f32 device formats are built (the task's conversion
//! pair); anything else is treated as unusable and falls through to the
//! default config, then to silence.
//!
//! Thread shape: cpal invokes the data callback on its own audio thread;
//! the callback locks the bus (an `Arc<Mutex<_>>` clone moved in) and
//! pulls mixed output — never blocking on the game thread beyond that
//! mutex. `_stream` keeps the stream alive: dropping it stops the
//! device. Errors go to the error callback (log and continue); a
//! device-open failure returns `Err` and play continues silent.
//!
//! Latency/dropout note: the device buffer size is cpal's default (the
//! backend picks it — typically a few hundred frames, i.e. single-digit
//! milliseconds at 44.1/48 kHz). Per callback sample the mixer does two
//! table lookups plus a handful of flops per voice — microseconds even
//! with dozens of stacked voices, far inside any default buffer. The
//! game-loop thread only ever holds the bus mutex for a register or a
//! flag flip, so callback-side contention is a non-issue; there is no
//! allocation or I/O on the audio path (the per-callback Vec reuse lives
//! in the closure captures).

use std::sync::{Arc, Mutex};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, Stream};
use nfm_core::audiobus::AudioBus;
use nfm_core::host::AudioSink;

/// A device output route, as enumerated: channel count, the rate range
/// the backend reports, and the sample format. Pure-data view of
/// `SupportedStreamConfigRange` so the choice is unit-testable.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Route {
    pub channels: u16,
    pub min_rate: u32,
    pub max_rate: u32,
    pub format: SampleFormat,
}

/// The chosen device route: an exact rate within the winning range.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChosenRoute {
    pub rate: u32,
    pub channels: u16,
    pub format: SampleFormat,
}

/// Pick the device route: exact 8000 Hz mono i16 first, then 8000 Hz
/// mono in any buildable format (i16 beats f32), then the closest rate
/// the range allows (`clamp(8000, min, max)`) mono-first, then the same
/// tiers for any channel count (stereo fallback: the callback duplicates
/// the mono mix). Returns `None` when nothing buildable is offered —
/// the caller then tries the device default, then silence.
pub fn choose_output(routes: &[Route]) -> Option<ChosenRoute> {
    // (mono-first, format rank) tiers, in order.
    for &(mono, formats) in &[
        (true, &[SampleFormat::I16][..]),
        (true, &[SampleFormat::F32][..]),
    ] {
        for r in routes {
            if (r.channels == 1) == mono
                && formats.contains(&r.format)
                && r.min_rate <= 8000
                && 8000 <= r.max_rate
            {
                return Some(ChosenRoute {
                    rate: 8000,
                    channels: r.channels,
                    format: r.format,
                });
            }
        }
    }
    for &mono in &[true, false] {
        for &format in &[SampleFormat::I16, SampleFormat::F32] {
            let mut best: Option<ChosenRoute> = None;
            for r in routes {
                if (r.channels == 1) != mono || r.format != format {
                    continue;
                }
                let rate = 8000u32.clamp(r.min_rate, r.max_rate);
                // Closest to 8000 wins; ties keep the first enumerated.
                let dist = rate.abs_diff(8000);
                let best_dist = best.map_or(u32::MAX, |b: ChosenRoute| b.rate.abs_diff(8000));
                if dist < best_dist {
                    best = Some(ChosenRoute {
                        rate,
                        channels: r.channels,
                        format,
                    });
                }
            }
            if best.is_some() {
                return best;
            }
        }
    }
    None
}

/// Lock the bus and mix into `out` — the one pull path, shared by the
/// device callback and the [`AudioSink`] impl below.
fn pull_bus(bus: &Mutex<AudioBus>, out: &mut [i16], rate: u32) {
    if let Ok(mut b) = bus.lock() {
        b.pull(out, rate);
    }
}

/// The default output device's name — the debug server's `/env` fact. A
/// missing device is a printable placeholder, never an error here.
#[cfg(debug_assertions)]
pub fn default_output_name() -> String {
    cpal::default_host()
        .default_output_device()
        .map(|device| device.to_string())
        .unwrap_or_else(|| "no default output device".to_string())
}

/// i16 mix sample → f32 device sample.
pub fn i16_to_f32(s: i16) -> f32 {
    // 32768.0 maps -32768 exactly to -1.0; 32767 lands a hair under 1.0.
    s as f32 / 32768.0
}

/// Spread a mono i16 mix across `channels` interleaved device slots.
pub fn spread_mono_i16(mono: &[i16], channels: usize, out: &mut [i16]) {
    for (i, &s) in mono.iter().enumerate() {
        for ch in 0..channels {
            out[i * channels + ch] = s;
        }
    }
}

/// Spread a mono i16 mix as interleaved f32 device slots.
pub fn spread_mono_f32(mono: &[i16], channels: usize, out: &mut [f32]) {
    for (i, &s) in mono.iter().enumerate() {
        let v = i16_to_f32(s);
        for ch in 0..channels {
            out[i * channels + ch] = v;
        }
    }
}

/// The cpal backend: the chosen device route plus the bus it drains. The
/// stream handle is kept alive here — dropping `CpalSink` stops the
/// device. (`_stream` is `None` only in unit tests, which drive
/// [`AudioSink::pull`] directly without hardware.)
pub struct CpalSink {
    bus: Arc<Mutex<AudioBus>>,
    rate: u32,
    channels: u16,
    format: SampleFormat,
    _stream: Option<Stream>,
}

impl CpalSink {
    /// Open the default output device on the preferred route and start
    /// the stream. `Err` carries a printable reason; play then runs
    /// silent (the `None`-audio path).
    pub fn open(bus: &Arc<Mutex<AudioBus>>) -> Result<Self, String> {
        let host = cpal::default_host();
        let device = host
            .default_output_device()
            .ok_or_else(|| "no default output device".to_string())?;
        let routes: Vec<Route> = device
            .supported_output_configs()
            .map_err(|e| format!("cannot list output configs: {e}"))?
            .map(|r| Route {
                channels: r.channels(),
                min_rate: r.min_sample_rate(),
                max_rate: r.max_sample_rate(),
                format: r.sample_format(),
            })
            .collect();
        let chosen = choose_output(&routes).or_else(|| {
            // Fallback chain: the device default config, if buildable.
            device.default_output_config().ok().and_then(|d| {
                let format = d.sample_format();
                if format != SampleFormat::I16 && format != SampleFormat::F32 {
                    return None;
                }
                Some(ChosenRoute {
                    rate: d.sample_rate(),
                    channels: d.channels(),
                    format,
                })
            })
        });
        let Some(chosen) = chosen else {
            return Err("no usable output config (need i16/f32)".to_string());
        };
        Self::open_route(bus, &device, chosen)
    }

    /// One-line `rate/channels/format` summary for the boot print.
    pub fn describe(&self) -> String {
        format!("{} Hz, {}ch, {:?}", self.rate, self.channels, self.format)
    }

    fn open_route(
        bus: &Arc<Mutex<AudioBus>>,
        device: &cpal::Device,
        chosen: ChosenRoute,
    ) -> Result<Self, String> {
        let config = cpal::StreamConfig {
            channels: chosen.channels,
            sample_rate: chosen.rate,
            buffer_size: cpal::BufferSize::Default,
        };
        let bus_cb = Arc::clone(bus);
        let rate = chosen.rate;
        let channels = chosen.channels as usize;
        let err_fn = |err| eprintln!("audio stream error: {err}");
        let stream = match chosen.format {
            SampleFormat::I16 => {
                let mut tmp: Vec<i16> = Vec::new();
                device
                    .build_output_stream(
                        config,
                        move |out: &mut [i16], _| {
                            // The bus pull is ADDITIVE (layered pulls
                            // compose), so the staging must start from
                            // zero every callback — a reused `resize`
                            // keeps the previous samples and the mix
                            // integrates into an exponential rail.
                            if channels == 1 {
                                for s in out.iter_mut() {
                                    *s = 0;
                                }
                                pull_bus(&bus_cb, out, rate);
                            } else {
                                let frames = out.len() / channels;
                                tmp.clear();
                                tmp.resize(frames, 0);
                                pull_bus(&bus_cb, &mut tmp, rate);
                                spread_mono_i16(&tmp, channels, out);
                            }
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|e| format!("cannot build i16 stream: {e}"))?
            }
            SampleFormat::F32 => {
                let mut tmp: Vec<i16> = Vec::new();
                let mut conv: Vec<f32> = Vec::new();
                device
                    .build_output_stream(
                        config,
                        move |out: &mut [f32], _| {
                            // Same additive-pull rule: zero the staging
                            // per callback (see the i16 arm).
                            let frames = out.len() / channels;
                            tmp.clear();
                            tmp.resize(frames, 0);
                            pull_bus(&bus_cb, &mut tmp, rate);
                            conv.resize(out.len(), 0.0);
                            spread_mono_f32(&tmp, channels, &mut conv);
                            out.copy_from_slice(&conv);
                        },
                        err_fn,
                        None,
                    )
                    .map_err(|e| format!("cannot build f32 stream: {e}"))?
            }
            _ => return Err("unreachable: chooser only emits i16/f32".to_string()),
        };
        stream
            .play()
            .map_err(|e| format!("cannot start stream: {e}"))?;
        Ok(Self {
            bus: Arc::clone(bus),
            rate: chosen.rate,
            channels: chosen.channels,
            format: chosen.format,
            _stream: Some(stream),
        })
    }
}

impl AudioSink for CpalSink {
    fn pull(&mut self, out: &mut [i16], rate: u32) {
        pull_bus(&self.bus, out, rate);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The device-callback contract: the bus pull is ADDITIVE (layered
    /// pulls compose), so the callback's staging must be zeroed per
    /// callback. The first listen's metallic buzz was exactly this: a
    /// reused staging buffer integrated the mix into an exponential
    /// rail (each callback added the new samples onto the old ones).
    /// This pins both halves of the contract — the additive semantic on
    /// the bus side and the zeroing obligation on the caller side.
    #[test]
    fn callback_staging_must_zero_before_pull() {
        use nfm_core::audiobus::VoiceMode;
        let bus = Arc::new(Mutex::new(AudioBus::new()));
        let data = Arc::new(vec![0x40i8; 100]);
        bus.lock()
            .unwrap()
            .register(data, 7900, 0.0, VoiceMode::Loop, true);

        // a pull into a FRESH (zeroed) buffer yields the mix alone
        let mut fresh = vec![0i16; 50];
        bus.lock().unwrap().pull(&mut fresh, 44100);
        let mix = fresh.clone();

        // the same pull into a DIRTY (non-zeroed) buffer integrates
        let mut dirty = vec![7i16; 50];
        bus.lock().unwrap().pull(&mut dirty, 44100);
        assert_ne!(dirty, mix, "a dirty staging integrates - the old bug");
        for (d, m) in dirty.iter().zip(mix.iter()) {
            assert_eq!(*d, 7 + m, "the additive semantic: prior + mix");
        }
    }

    fn route(channels: u16, min_rate: u32, max_rate: u32, format: SampleFormat) -> Route {
        Route {
            channels,
            min_rate,
            max_rate,
            format,
        }
    }

    /// The tier order: exact 8000 mono i16 first, then 8000 mono f32,
    /// then clamped mono (either direction), then any channel count.
    #[test]
    fn choose_output_prefers_8000_mono_i16() {
        // exact match wins over everything else on offer
        let routes = vec![
            route(2, 44100, 48000, SampleFormat::I16),
            route(1, 44100, 48000, SampleFormat::F32),
            route(1, 8000, 8000, SampleFormat::I16),
        ];
        assert_eq!(
            choose_output(&routes),
            Some(ChosenRoute {
                rate: 8000,
                channels: 1,
                format: SampleFormat::I16,
            })
        );

        // 8000 mono but only f32: rate stays, format follows
        let routes = vec![
            route(2, 8000, 8000, SampleFormat::I16),
            route(1, 8000, 8000, SampleFormat::F32),
        ];
        assert_eq!(
            choose_output(&routes),
            Some(ChosenRoute {
                rate: 8000,
                channels: 1,
                format: SampleFormat::F32,
            })
        );

        // max below 8000: take the top of the range
        let routes = vec![route(1, 4000, 4000, SampleFormat::I16)];
        assert_eq!(
            choose_output(&routes),
            Some(ChosenRoute {
                rate: 4000,
                channels: 1,
                format: SampleFormat::I16,
            })
        );

        // min above 8000: take the bottom of the range
        let routes = vec![route(1, 44100, 48000, SampleFormat::I16)];
        assert_eq!(
            choose_output(&routes),
            Some(ChosenRoute {
                rate: 44100,
                channels: 1,
                format: SampleFormat::I16,
            })
        );

        // stereo-only device: duplicate the mono mix
        let routes = vec![route(2, 44100, 44100, SampleFormat::I16)];
        assert_eq!(
            choose_output(&routes),
            Some(ChosenRoute {
                rate: 44100,
                channels: 2,
                format: SampleFormat::I16,
            })
        );

        // nothing buildable (exotic formats only) or nothing at all
        assert_eq!(
            choose_output(&[route(1, 8000, 8000, SampleFormat::U16)]),
            None
        );
        let empty: Vec<Route> = vec![];
        assert_eq!(choose_output(&empty), None);
    }

    /// i16 → f32 device samples: silence stays silent, extremes map to
    /// the rails.
    #[test]
    fn i16_to_f32_conversion() {
        assert_eq!(i16_to_f32(0), 0.0);
        assert_eq!(i16_to_f32(-32768), -1.0);
        assert!((i16_to_f32(32767) - 0.9999695).abs() < 1e-6);
    }

    /// Mono spreads to every channel slot, interleaved.
    #[test]
    fn spread_mono_fills_all_channels() {
        let mono = [100i16, -200];
        let mut stereo = [0i16; 4];
        spread_mono_i16(&mono, 2, &mut stereo);
        assert_eq!(stereo, [100, 100, -200, -200]);
        let mut f = [0.0f32; 4];
        spread_mono_f32(&mono, 2, &mut f);
        assert_eq!(
            f,
            [
                i16_to_f32(100),
                i16_to_f32(100),
                i16_to_f32(-200),
                i16_to_f32(-200)
            ]
        );
    }

    /// The sink pulls the bus through the AudioSink seam without hardware:
    /// silence in → silence out, and a 2-sample OneShot is consumed past
    /// the end (proving the pull reached the voice).
    #[test]
    fn sink_pull_mixes_without_hardware() {
        use nfm_core::audiobus::{AudioBus, VoiceMode};
        use std::sync::{Arc, Mutex};

        let bus = Arc::new(Mutex::new(AudioBus::new()));
        {
            let mut b = bus.lock().unwrap();
            // 0xFF decodes to 0 (table entry 0 is -1 → linear 0)
            let idx = b.register(Arc::new(vec![-1i8; 8]), 8000, 0.0, VoiceMode::Loop, true);
            assert_eq!(idx, 0);
            let idx2 = b.register(Arc::new(vec![-1i8; 2]), 8000, 0.0, VoiceMode::OneShot, true);
            assert_eq!(idx2, 1);
        }
        let mut sink = CpalSink {
            bus: Arc::clone(&bus),
            rate: 8000,
            channels: 1,
            format: SampleFormat::I16,
            _stream: None,
        };
        let mut out = [0i16; 4];
        AudioSink::pull(&mut sink, &mut out, 8000);
        assert_eq!(out, [0, 0, 0, 0], "silence in, silence out");
        assert!(
            !bus.lock().unwrap().is_playing(1),
            "the OneShot was consumed past its end"
        );
        assert_eq!(sink.describe(), "8000 Hz, 1ch, I16");
    }
}
