//! cpal-based audio recorder implementation

use super::Recorder;
use anyhow::{Context, Result};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{Sample, SampleFormat, Stream};
use hound::{SampleFormat as HoundSampleFormat, WavSpec, WavWriter};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

/// Audio recorder using cpal
pub struct CpalRecorder {
    /// Directory for temporary audio files
    output_dir: PathBuf,
    /// Current stream (if recording)
    stream: Option<Stream>,
    /// Collected samples
    samples: Arc<Mutex<Vec<f32>>>,
    /// Recording state flag
    recording: Arc<AtomicBool>,
    /// Current audio level for visualization
    level: Arc<Mutex<f32>>,
    /// Sample rate of the recording
    sample_rate: u32,
    /// Number of channels
    channels: u16,
    /// Path to the output file (set after stop)
    output_path: Option<PathBuf>,
}

impl CpalRecorder {
    /// Create a new recorder that saves files to the given directory
    pub fn new(output_dir: PathBuf) -> Self {
        Self {
            output_dir,
            stream: None,
            samples: Arc::new(Mutex::new(Vec::new())),
            recording: Arc::new(AtomicBool::new(false)),
            level: Arc::new(Mutex::new(0.0)),
            sample_rate: 0,
            channels: 0,
            output_path: None,
        }
    }

    /// Create a recorder using the system temp directory
    pub fn with_temp_dir() -> Self {
        Self::new(std::env::temp_dir())
    }

    /// Get the device name being used for recording
    #[allow(dead_code)]
    pub fn device_name(&self) -> Result<String> {
        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("No input device available")?;
        device.name().context("Failed to get device name")
    }
}

impl Recorder for CpalRecorder {
    fn start(&mut self) -> Result<()> {
        if self.is_recording() {
            anyhow::bail!("Already recording");
        }

        let host = cpal::default_host();
        let device = host
            .default_input_device()
            .context("No input device available")?;

        let config = device.default_input_config()?;
        self.sample_rate = config.sample_rate().0;
        self.channels = config.channels();

        // Clear previous samples
        self.samples.lock().unwrap().clear();
        self.recording.store(true, Ordering::SeqCst);

        let samples = self.samples.clone();
        let recording = self.recording.clone();
        let level = self.level.clone();

        let err_fn = |err| eprintln!("Audio stream error: {}", err);

        let stream = match config.sample_format() {
            SampleFormat::F32 => {
                let samples = samples.clone();
                let recording = recording.clone();
                let level = level.clone();
                device.build_input_stream(
                    &config.into(),
                    move |data: &[f32], _| {
                        if recording.load(Ordering::SeqCst) {
                            // Calculate RMS level for visualization
                            let rms = (data.iter().map(|&s| s * s).sum::<f32>()
                                / data.len() as f32)
                                .sqrt();
                            *level.lock().unwrap() = rms.min(1.0);

                            samples.lock().unwrap().extend_from_slice(data);
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            SampleFormat::I16 => {
                let samples = samples.clone();
                let recording = recording.clone();
                let level = level.clone();
                device.build_input_stream(
                    &config.into(),
                    move |data: &[i16], _| {
                        if recording.load(Ordering::SeqCst) {
                            let floats: Vec<f32> =
                                data.iter().map(|&s| f32::from_sample(s)).collect();

                            // Calculate RMS level
                            let rms = (floats.iter().map(|&s| s * s).sum::<f32>()
                                / floats.len() as f32)
                                .sqrt();
                            *level.lock().unwrap() = rms.min(1.0);

                            samples.lock().unwrap().extend(floats);
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            SampleFormat::U16 => {
                let samples = samples.clone();
                let recording = recording.clone();
                let level = level.clone();
                device.build_input_stream(
                    &config.into(),
                    move |data: &[u16], _| {
                        if recording.load(Ordering::SeqCst) {
                            let floats: Vec<f32> =
                                data.iter().map(|&s| f32::from_sample(s)).collect();

                            // Calculate RMS level
                            let rms = (floats.iter().map(|&s| s * s).sum::<f32>()
                                / floats.len() as f32)
                                .sqrt();
                            *level.lock().unwrap() = rms.min(1.0);

                            samples.lock().unwrap().extend(floats);
                        }
                    },
                    err_fn,
                    None,
                )?
            }
            format => anyhow::bail!("Unsupported sample format: {:?}", format),
        };

        stream.play()?;
        self.stream = Some(stream);
        self.output_path = None;

        Ok(())
    }

    fn stop(&mut self) -> Result<PathBuf> {
        if !self.is_recording() {
            anyhow::bail!("Not currently recording");
        }

        // Stop recording
        self.recording.store(false, Ordering::SeqCst);

        // Drop the stream to stop audio capture
        self.stream = None;

        // Get samples and convert to mono if needed
        let samples = self.samples.lock().unwrap();
        let mono_samples: Vec<f32> = if self.channels > 1 {
            samples
                .chunks(self.channels as usize)
                .map(|chunk| chunk.iter().sum::<f32>() / self.channels as f32)
                .collect()
        } else {
            samples.clone()
        };

        // Resample to 16kHz (required by transcription backends)
        let target_rate = 16000u32;
        let (final_samples, output_rate) = if self.sample_rate != target_rate {
            (
                resample(&mono_samples, self.sample_rate, target_rate),
                target_rate,
            )
        } else {
            (mono_samples, self.sample_rate)
        };

        // Generate output path
        let output_path = self.output_dir.join(format!(
            "sweet-nothings-{}-{}.wav",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_millis()
        ));

        // Write WAV file
        let spec = WavSpec {
            channels: 1,
            sample_rate: output_rate,
            bits_per_sample: 32,
            sample_format: HoundSampleFormat::Float,
        };

        let mut writer = WavWriter::create(&output_path, spec)?;
        for sample in final_samples.iter() {
            writer.write_sample(*sample)?;
        }
        writer.finalize()?;

        self.output_path = Some(output_path.clone());

        Ok(output_path)
    }

    fn is_recording(&self) -> bool {
        self.recording.load(Ordering::SeqCst)
    }

    fn audio_level(&self) -> f32 {
        *self.level.lock().unwrap()
    }
}

/// Resample audio using linear interpolation.
fn resample(samples: &[f32], from_rate: u32, to_rate: u32) -> Vec<f32> {
    let ratio = from_rate as f64 / to_rate as f64;
    let output_len = (samples.len() as f64 / ratio) as usize;
    let mut output = Vec::with_capacity(output_len);

    for i in 0..output_len {
        let src_pos = i as f64 * ratio;
        let src_idx = src_pos as usize;
        let frac = src_pos - src_idx as f64;

        let sample = if src_idx + 1 < samples.len() {
            samples[src_idx] as f64 * (1.0 - frac) + samples[src_idx + 1] as f64 * frac
        } else if src_idx < samples.len() {
            samples[src_idx] as f64
        } else {
            0.0
        };
        output.push(sample as f32);
    }

    output
}

impl Drop for CpalRecorder {
    fn drop(&mut self) {
        // Stop recording if still active
        if self.is_recording() {
            self.recording.store(false, Ordering::SeqCst);
        }
    }
}
