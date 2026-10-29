use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::mpsc::{Receiver, Sender, SyncSender, channel, sync_channel};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use crossbeam_queue::ArrayQueue;

use super::AudioError;
use super::event::AudioCommand;
use super::synth::SoundFontSynth;

/// Commands the callback can hold waiting at once.
const COMMAND_CAPACITY: usize = 1024;

/// Frames rendered per pass; a larger device buffer is rendered in slices.
const MAX_BLOCK_FRAMES: usize = 2048;

/// A request to load a SoundFont into the thread that owns the audio stream.
struct AudioRequest {
    path: PathBuf,
    commands: Arc<ArrayQueue<AudioCommand>>,
    reply: SyncSender<Result<(), AudioError>>,
}

/// Owns the audio thread. Dropping it stops playback.
pub struct AudioEngine {
    requests: Sender<AudioRequest>,
    commands: Option<Arc<ArrayQueue<AudioCommand>>>,
}

impl AudioEngine {
    /// Starts the audio thread. It stays silent until a SoundFont is loaded.
    pub fn new() -> Self {
        let (requests, receiver) = channel();
        std::thread::Builder::new()
            .name("audio".to_string())
            .spawn(move || audio_loop(receiver))
            .expect("the audio thread should start");
        AudioEngine {
            requests,
            commands: None,
        }
    }

    /// Loads a SoundFont, replacing any current one. The engine goes silent if it fails.
    pub fn load_soundfont(&mut self, path: &Path) -> Result<(), AudioError> {
        let commands = Arc::new(ArrayQueue::new(COMMAND_CAPACITY));
        let (reply, response) = sync_channel(1);
        self.requests
            .send(AudioRequest {
                path: path.to_path_buf(),
                commands: Arc::clone(&commands),
                reply,
            })
            .map_err(|_| AudioError::Stopped)?;

        match response.recv() {
            Ok(Ok(())) => {
                self.commands = Some(commands);
                Ok(())
            }
            Ok(Err(err)) => {
                self.commands = None;
                Err(err)
            }
            Err(_) => {
                self.commands = None;
                Err(AudioError::Stopped)
            }
        }
    }

    /// Whether this engine is running without a SoundFont.
    pub fn is_silent(&self) -> bool {
        self.commands.is_none()
    }

    /// Queues a command for the next callback. Dropped when silent or when the queue is full.
    pub fn send(&self, command: AudioCommand) {
        if let Some(commands) = &self.commands {
            let _ = commands.push(command);
        }
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        AudioEngine::new()
    }
}

/// Owns the output stream. cpal's `Stream` is not `Send`, so it never leaves this thread.
fn audio_loop(requests: Receiver<AudioRequest>) {
    let mut stream: Option<cpal::Stream> = None;
    while let Ok(request) = requests.recv() {
        // Drop the previous stream before opening the new one.
        drop(stream.take());
        match build_stream(&request.path, &request.commands) {
            Ok(new_stream) => {
                stream = Some(new_stream);
                let _ = request.reply.send(Ok(()));
            }
            Err(err) => {
                let _ = request.reply.send(Err(err));
            }
        }
    }
}

fn build_stream(
    path: &Path,
    commands: &Arc<ArrayQueue<AudioCommand>>,
) -> Result<cpal::Stream, AudioError> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or(AudioError::NoOutputDevice)?;
    let (config, sample_rate) = output_config(&device)?;

    let consumer = Arc::clone(commands);
    let channels = usize::from(config.channels);
    let mut synth = SoundFontSynth::load(path, sample_rate)?;
    let mut left = vec![0_f32; MAX_BLOCK_FRAMES];
    let mut right = vec![0_f32; MAX_BLOCK_FRAMES];

    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                while let Some(command) = consumer.pop() {
                    synth.apply(command);
                }
                for chunk in data.chunks_mut(MAX_BLOCK_FRAMES * channels) {
                    let frames = chunk.len() / channels;
                    let left = &mut left[..frames];
                    let right = &mut right[..frames];
                    synth.render(left, right);
                    write_frames(left, right, channels, chunk);
                }
            },
            |err| log::error!(target: "audio", "output stream error: {err}"),
            None,
        )
        .map_err(device_error)?;

    stream.play().map_err(device_error)?;
    Ok(stream)
}

/// Picks a 32-bit float output config, preferring the device default.
fn output_config(device: &cpal::Device) -> Result<(StreamConfig, u32), AudioError> {
    let default = device.default_output_config().map_err(device_error)?;
    if default.sample_format() == SampleFormat::F32 {
        let sample_rate = default.sample_rate().0;
        return Ok((default.config(), sample_rate));
    }
    for range in device.supported_output_configs().map_err(device_error)? {
        if range.sample_format() == SampleFormat::F32 {
            let config = range.with_max_sample_rate().config();
            let sample_rate = config.sample_rate.0;
            return Ok((config, sample_rate));
        }
    }
    Err(AudioError::UnsupportedFormat)
}

/// Writes one rendered frame into the device's interleaved samples.
fn write_frames(left: &[f32], right: &[f32], channels: usize, out: &mut [f32]) {
    for (frame, samples) in out.chunks_mut(channels).enumerate() {
        if channels == 1 {
            samples[0] = (left[frame] + right[frame]) * 0.5;
        } else {
            samples[0] = left[frame];
            samples[1] = right[frame];
            for extra in &mut samples[2..] {
                *extra = 0.0;
            }
        }
    }
}

fn device_error(error: impl std::fmt::Display) -> AudioError {
    AudioError::Device(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::write_frames;

    #[test]
    fn interleaves_stereo_frames() {
        let left = [0.1, 0.3];
        let right = [0.2, 0.4];
        let mut out = [0.0; 4];

        write_frames(&left, &right, 2, &mut out);

        assert_eq!(out, [0.1, 0.2, 0.3, 0.4]);
    }

    #[test]
    fn mixes_to_mono_and_quiets_extra_channels() {
        let left = [0.5, 1.0];
        let right = [0.1, 0.0];

        let mut mono = [0.0; 2];
        write_frames(&left, &right, 1, &mut mono);
        assert_eq!(mono, [0.3, 0.5]);

        let mut surround = [9.0; 6];
        write_frames(&left, &right, 3, &mut surround);
        assert_eq!(surround, [0.5, 0.1, 0.0, 1.0, 0.0, 0.0]);
    }
}
