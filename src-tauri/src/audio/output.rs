use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, SyncSender, channel, sync_channel};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use crossbeam_queue::ArrayQueue;

use super::AudioError;
use super::event::{AudioCommand, PlaybackControl, ScheduledCommand};
use super::synth::{SoundFontSynth, Synth};

/// Commands the callback can hold waiting at once.
const COMMAND_CAPACITY: usize = 1024;

/// Playback changes the callback can hold waiting at once.
const CONTROL_CAPACITY: usize = 64;

/// Frames rendered per pass; a larger device buffer is rendered in slices.
const MAX_BLOCK_FRAMES: usize = 2048;

/// Tempo range accepted from the outside, as a percentage.
const MIN_TEMPO_PERCENT: f32 = 10.0;
const MAX_TEMPO_PERCENT: f32 = 400.0;

/// A request to load a SoundFont into the thread that owns the audio stream.
struct AudioRequest {
    path: PathBuf,
    control: Arc<ArrayQueue<PlaybackControl>>,
    position: Arc<AtomicU64>,
    reply: SyncSender<Result<(), AudioError>>,
}

/// Owns the audio thread. Dropping it stops playback.
pub struct AudioEngine {
    requests: Sender<AudioRequest>,
    /// Stable queue for commands applied immediately (live input). Never swapped, so a MIDI or
    /// keyboard thread can hold an [`InputSink`] to it without ever touching the engine lock.
    commands: Arc<ArrayQueue<AudioCommand>>,
    control: Option<Arc<ArrayQueue<PlaybackControl>>>,
    position: Arc<AtomicU64>,
}

/// A cloneable, lock-free handle for pushing live input into the audio thread without taking the
/// engine lock. Safe to keep on a MIDI input or keyboard thread.
#[derive(Clone)]
pub struct InputSink {
    commands: Arc<ArrayQueue<AudioCommand>>,
}

impl InputSink {
    /// Queues a command, dropping it if the queue is full.
    pub fn send(&self, command: AudioCommand) {
        let _ = self.commands.push(command);
    }
}

impl AudioEngine {
    /// Starts the audio thread. It stays silent until a SoundFont is loaded.
    pub fn new() -> Self {
        let (requests, receiver) = channel();
        let commands = Arc::new(ArrayQueue::new(COMMAND_CAPACITY));
        let audio_commands = Arc::clone(&commands);
        std::thread::Builder::new()
            .name("audio".to_string())
            .spawn(move || audio_loop(receiver, audio_commands))
            .expect("the audio thread should start");
        AudioEngine {
            requests,
            commands,
            control: None,
            position: Arc::new(AtomicU64::new(0)),
        }
    }

    /// Loads a SoundFont, replacing any current one. The engine goes silent if it fails.
    pub fn load_soundfont(&mut self, path: &Path) -> Result<(), AudioError> {
        let control = Arc::new(ArrayQueue::new(CONTROL_CAPACITY));
        let position = Arc::new(AtomicU64::new(0));
        let (reply, response) = sync_channel(1);
        self.requests
            .send(AudioRequest {
                path: path.to_path_buf(),
                control: Arc::clone(&control),
                position: Arc::clone(&position),
                reply,
            })
            .map_err(|_| AudioError::Stopped)?;

        match response.recv() {
            Ok(Ok(())) => {
                self.control = Some(control);
                self.position = position;
                // Drop anything queued while there was no stream to drain it.
                while self.commands.pop().is_some() {}
                Ok(())
            }
            Ok(Err(err)) => {
                self.silence();
                Err(err)
            }
            Err(_) => {
                self.silence();
                Err(AudioError::Stopped)
            }
        }
    }

    /// Whether this engine is running without a SoundFont.
    pub fn is_silent(&self) -> bool {
        self.control.is_none()
    }

    /// A handle any input thread can hold to feed live notes in.
    pub fn input_sink(&self) -> InputSink {
        InputSink {
            commands: Arc::clone(&self.commands),
        }
    }

    /// Queues a command to play immediately, for live input. Dropped when the queue is full.
    pub fn send(&self, command: AudioCommand) {
        self.input_sink().send(command);
    }

    /// Replaces the piece being played and starts it from the beginning.
    pub fn set_score(&self, score: Arc<Vec<ScheduledCommand>>) {
        self.control(PlaybackControl::SetScore(score));
    }

    pub fn start(&self) {
        self.control(PlaybackControl::Start);
    }

    pub fn stop(&self) {
        self.control(PlaybackControl::Stop);
    }

    /// Sets the playback tempo as a percentage of the original.
    pub fn set_tempo_percent(&self, percent: f32) {
        self.control(PlaybackControl::SetTempoPercent(percent));
    }

    /// Shifts the piece up (positive) or down (negative) by up to 12 semitones.
    pub fn set_transpose(&self, semitones: i32) {
        self.control(PlaybackControl::SetTranspose(semitones));
    }

    /// Position of the playhead in the current piece, in milliseconds.
    pub fn position_ms(&self) -> u64 {
        self.position.load(Ordering::Relaxed)
    }

    fn control(&self, message: PlaybackControl) {
        if let Some(control) = &self.control {
            let _ = control.push(message);
        }
    }

    fn silence(&mut self) {
        self.control = None;
        self.position = Arc::new(AtomicU64::new(0));
    }
}

impl Default for AudioEngine {
    fn default() -> Self {
        AudioEngine::new()
    }
}

/// Owns the output stream. cpal's `Stream` is not `Send`, so it never leaves this thread.
fn audio_loop(requests: Receiver<AudioRequest>, commands: Arc<ArrayQueue<AudioCommand>>) {
    let mut stream: Option<cpal::Stream> = None;
    while let Ok(request) = requests.recv() {
        // Drop the previous stream before opening the new one.
        drop(stream.take());
        match build_stream(&request, &commands) {
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
    request: &AudioRequest,
    commands: &Arc<ArrayQueue<AudioCommand>>,
) -> Result<cpal::Stream, AudioError> {
    let host = cpal::default_host();
    let device = host
        .default_output_device()
        .ok_or(AudioError::NoOutputDevice)?;
    let (config, sample_rate) = output_config(&device)?;
    let channels = usize::from(config.channels);

    let mut renderer = Renderer {
        synth: SoundFontSynth::load(&request.path, sample_rate)?,
        commands: Arc::clone(commands),
        control: Arc::clone(&request.control),
        position: Arc::clone(&request.position),
        channels,
        sample_rate,
        left: vec![0_f32; MAX_BLOCK_FRAMES],
        right: vec![0_f32; MAX_BLOCK_FRAMES],
        playback: ScorePlayback::default(),
    };

    let stream = device
        .build_output_stream(
            &config,
            move |data: &mut [f32], _: &cpal::OutputCallbackInfo| {
                for chunk in data.chunks_mut(MAX_BLOCK_FRAMES * channels) {
                    renderer.render_block(chunk);
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

/// The piece being played and how far through it we are.
struct ScorePlayback {
    score: Arc<Vec<ScheduledCommand>>,
    index: usize,
    position_ms: f64,
    tempo_percent: f32,
    transpose: i32,
    playing: bool,
}

impl Default for ScorePlayback {
    fn default() -> Self {
        ScorePlayback {
            score: Arc::new(Vec::new()),
            index: 0,
            position_ms: 0.0,
            // Normal speed. A zero here would make the frame conversion divide by zero.
            tempo_percent: 100.0,
            transpose: 0,
            playing: false,
        }
    }
}

impl ScorePlayback {
    /// Score time of the next command, or infinity when the piece is finished.
    fn next_ms(&self) -> f64 {
        self.score
            .get(self.index)
            .map_or(f64::INFINITY, |event| event.at_ms)
    }

    fn advance(&mut self, synth: &mut impl Synth) {
        if let Some(event) = self.score.get(self.index) {
            synth.apply(transposed(event.command, self.transpose));
            self.index += 1;
        }
    }
}

/// Renders blocks and applies score commands at their exact frame.
struct Renderer<S: Synth> {
    synth: S,
    commands: Arc<ArrayQueue<AudioCommand>>,
    control: Arc<ArrayQueue<PlaybackControl>>,
    position: Arc<AtomicU64>,
    channels: usize,
    sample_rate: u32,
    playback: ScorePlayback,
    left: Vec<f32>,
    right: Vec<f32>,
}

impl<S: Synth> Renderer<S> {
    /// Renders one block of at most `MAX_BLOCK_FRAMES` frames into `data`.
    fn render_block(&mut self, data: &mut [f32]) {
        while let Some(message) = self.control.pop() {
            self.apply_control(message);
        }
        while let Some(command) = self.commands.pop() {
            self.synth.apply(command);
        }

        let frames = data.len() / self.channels;
        if self.playback.playing && self.sample_rate > 0 && !self.playback.score.is_empty() {
            self.render_score(data, frames);
        } else {
            self.render_segment(data, 0, frames);
        }

        self.position
            .store(self.playback.position_ms.max(0.0) as u64, Ordering::Relaxed);
    }

    fn apply_control(&mut self, message: PlaybackControl) {
        match message {
            PlaybackControl::SetScore(score) => {
                self.playback.score = score;
                self.playback.index = 0;
                self.playback.position_ms = 0.0;
            }
            PlaybackControl::SetTempoPercent(percent) => {
                self.playback.tempo_percent = percent.clamp(MIN_TEMPO_PERCENT, MAX_TEMPO_PERCENT);
            }
            PlaybackControl::SetTranspose(semitones) => {
                self.playback.transpose = semitones.clamp(-12, 12);
            }
            PlaybackControl::Start => self.playback.playing = true,
            PlaybackControl::Stop => {
                self.playback.playing = false;
                self.synth.apply(AudioCommand::AllNotesOff);
            }
        }
    }

    /// Renders `frames`, applying each score command at its exact sample offset.
    fn render_score(&mut self, data: &mut [f32], frames: usize) {
        // Score milliseconds per output frame. Faster tempo (higher percent) means less time per
        // frame. `tempo_percent` is clamped above zero, so this is always finite and positive.
        let ms_per_frame =
            1000.0 * f64::from(self.playback.tempo_percent) / (100.0 * f64::from(self.sample_rate));

        let mut rendered = 0;
        while rendered < frames {
            let remaining = frames - rendered;
            let end_ms = self.playback.position_ms + remaining as f64 * ms_per_frame;
            let next_ms = self.playback.next_ms();

            if next_ms >= end_ms {
                self.render_segment(data, rendered, remaining);
                self.playback.position_ms = end_ms;
                rendered = frames;
            } else {
                let gap_ms = (next_ms - self.playback.position_ms).max(0.0);
                let advance = ((gap_ms / ms_per_frame).round() as usize).min(remaining);
                self.render_segment(data, rendered, advance);
                self.playback.position_ms += advance as f64 * ms_per_frame;
                rendered += advance;
                // `next_ms` is finite here, so there is an event to consume: `index` advances and
                // the loop makes progress even when `advance` rounds down to zero.
                self.playback.advance(&mut self.synth);
            }
        }
    }

    fn render_segment(&mut self, data: &mut [f32], offset: usize, count: usize) {
        let left = &mut self.left[..count];
        let right = &mut self.right[..count];
        self.synth.render(left, right);
        let start = offset * self.channels;
        let end = start + count * self.channels;
        write_frames(
            &self.left[..count],
            &self.right[..count],
            self.channels,
            &mut data[start..end],
        );
    }
}

/// Shifts a note command's pitch, clamping to the MIDI range.
fn transposed(command: AudioCommand, semitones: i32) -> AudioCommand {
    if semitones == 0 {
        return command;
    }
    match command {
        AudioCommand::NoteOn {
            channel,
            pitch,
            velocity,
        } => AudioCommand::NoteOn {
            channel,
            pitch: shift(pitch, semitones),
            velocity,
        },
        AudioCommand::NoteOff { channel, pitch } => AudioCommand::NoteOff {
            channel,
            pitch: shift(pitch, semitones),
        },
        other => other,
    }
}

fn shift(pitch: u8, semitones: i32) -> u8 {
    (i32::from(pitch) + semitones).clamp(0, 127) as u8
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
    use super::{MAX_BLOCK_FRAMES, Renderer, ScorePlayback, shift, transposed, write_frames};
    use crate::audio::event::{AudioCommand, PlaybackControl, ScheduledCommand};
    use crate::audio::synth::Synth;
    use crossbeam_queue::ArrayQueue;
    use std::sync::Arc;
    use std::sync::atomic::AtomicU64;

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

    #[test]
    fn shifts_pitches_and_clamps() {
        assert_eq!(shift(60, 12), 72);
        assert_eq!(shift(60, -12), 48);
        assert_eq!(shift(2, -12), 0);
        assert_eq!(shift(126, 12), 127);
    }

    #[test]
    fn transpose_only_touches_notes() {
        assert_eq!(
            transposed(
                AudioCommand::NoteOn {
                    channel: 0,
                    pitch: 60,
                    velocity: 100
                },
                2
            ),
            AudioCommand::NoteOn {
                channel: 0,
                pitch: 62,
                velocity: 100
            }
        );
        assert_eq!(
            transposed(
                AudioCommand::ProgramChange {
                    channel: 0,
                    program: 40
                },
                2
            ),
            AudioCommand::ProgramChange {
                channel: 0,
                program: 40
            }
        );
    }

    #[test]
    fn applies_score_commands_at_their_exact_frame() {
        // A sample rate of 1000 makes one millisecond equal one frame at normal tempo.
        let mut renderer = renderer(
            1000,
            vec![scheduled(0.0, note_on(60)), scheduled(500.0, note_off(60))],
        );
        renderer.push_control(PlaybackControl::Start);

        renderer.render(1000);

        assert_eq!(
            renderer.synth.events,
            vec![(0, note_on(60)), (500, note_off(60))]
        );
        assert_eq!(renderer.published_ms(), 1000);
    }

    #[test]
    fn tempo_percent_stretches_the_timeline() {
        // 200% halves the frames per score millisecond.
        let mut renderer = renderer(1000, vec![scheduled(500.0, note_off(60))]);
        renderer.push_control(PlaybackControl::SetTempoPercent(200.0));
        renderer.push_control(PlaybackControl::Start);

        renderer.render(1000);

        assert_eq!(renderer.synth.events, vec![(250, note_off(60))]);
    }

    #[test]
    fn transpose_moves_score_pitches() {
        let mut renderer = renderer(1000, vec![scheduled(0.0, note_on(60))]);
        renderer.push_control(PlaybackControl::SetTranspose(5));
        renderer.push_control(PlaybackControl::Start);

        renderer.render(1);

        assert_eq!(renderer.synth.events, vec![(0, note_on(65))]);
    }

    #[test]
    fn defers_commands_past_the_block() {
        let mut renderer = renderer(
            1000,
            vec![scheduled(0.0, note_on(60)), scheduled(500.0, note_off(60))],
        );
        renderer.push_control(PlaybackControl::Start);

        renderer.render(100);
        assert_eq!(renderer.synth.events, vec![(0, note_on(60))]);

        renderer.render(450);
        assert_eq!(
            renderer.synth.events,
            vec![(0, note_on(60)), (500, note_off(60))]
        );
    }

    #[test]
    fn stop_silences_notes_and_halts_the_position() {
        let mut renderer = renderer(1000, vec![scheduled(0.0, note_on(60))]);
        renderer.push_control(PlaybackControl::Start);
        renderer.render(10);

        renderer.push_control(PlaybackControl::Stop);
        renderer.render(10);

        assert_eq!(
            renderer.synth.events.last(),
            Some(&(10, AudioCommand::AllNotesOff))
        );
        assert_eq!(renderer.published_ms(), 10);
    }

    /// A `Synth` that records the frame each command arrived at.
    #[derive(Default)]
    struct FakeSynth {
        rendered: usize,
        events: Vec<(usize, AudioCommand)>,
    }

    impl Synth for FakeSynth {
        fn apply(&mut self, command: AudioCommand) {
            self.events.push((self.rendered, command));
        }

        fn render(&mut self, left: &mut [f32], _right: &mut [f32]) {
            self.rendered += left.len();
        }
    }

    impl Renderer<FakeSynth> {
        fn push_control(&self, message: PlaybackControl) {
            self.control
                .push(message)
                .expect("control queue should fit");
        }

        fn render(&mut self, frames: usize) {
            let mut data = vec![0.0; frames * self.channels];
            self.render_block(&mut data);
        }

        fn published_ms(&self) -> u64 {
            self.position.load(std::sync::atomic::Ordering::Relaxed)
        }
    }

    fn renderer(sample_rate: u32, score: Vec<ScheduledCommand>) -> Renderer<FakeSynth> {
        Renderer {
            synth: FakeSynth::default(),
            commands: Arc::new(ArrayQueue::new(16)),
            control: Arc::new(ArrayQueue::new(16)),
            position: Arc::new(AtomicU64::new(0)),
            channels: 2,
            sample_rate,
            playback: ScorePlayback {
                score: Arc::new(score),
                ..ScorePlayback::default()
            },
            left: vec![0.0; MAX_BLOCK_FRAMES],
            right: vec![0.0; MAX_BLOCK_FRAMES],
        }
    }

    fn scheduled(at_ms: f64, command: AudioCommand) -> ScheduledCommand {
        ScheduledCommand { at_ms, command }
    }

    fn note_on(pitch: u8) -> AudioCommand {
        AudioCommand::NoteOn {
            channel: 0,
            pitch,
            velocity: 100,
        }
    }

    fn note_off(pitch: u8) -> AudioCommand {
        AudioCommand::NoteOff { channel: 0, pitch }
    }
}
