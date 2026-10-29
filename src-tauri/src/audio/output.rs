use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, SyncSender, channel, sync_channel};

use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{SampleFormat, StreamConfig};
use crossbeam_queue::ArrayQueue;

use super::AudioError;
use super::event::{AudioCommand, Gate, PlaybackControl, PlayedNote, ScheduledCommand};
use super::synth::{SoundFontSynth, Synth};
use super::{
    CUE_CHANNEL, CUE_PITCH, CUE_PROGRAM, CUE_VELOCITY, MAX_TEMPO_PERCENT, MIN_TEMPO_PERCENT,
    PLAYER_CHANNEL, TRANSPOSE_SEMITONES,
};

/// Commands the callback can hold waiting at once.
const COMMAND_CAPACITY: usize = 1024;

/// Playback changes the callback can hold waiting at once.
const CONTROL_CAPACITY: usize = 64;

/// Frames rendered per pass; a larger device buffer is rendered in slices.
const MAX_BLOCK_FRAMES: usize = 2048;

/// How early the player may play a gate's note and still release it. Matches the scoring window.
const FOLLOW_LEAD_MS: f64 = 250.0;

/// How far the playhead may run past an unplayed gate before follow-you mode waits for it. This
/// is the slack that keeps the accompaniment flowing instead of stopping at every single note.
/// It must stay within the matcher's window (`scoring::DEFAULT_WINDOW_MS`), or a correct note
/// played at the limit would be scored as wrong and dropped by the matcher.
const FOLLOW_TOLERANCE_MS: f64 = 250.0;

/// The MIDI controller for channel volume, the SoundFont synth's per-source gain.
const VOLUME_CONTROLLER: u8 = 7;

/// Master make-up gain applied after synthesis, on top of the Output slider. rustysynth renders
/// most SoundFonts well below full scale, so this brings the app up to a usable level; samples
/// are clamped afterwards so a loud moment distorts rather than overflows.
const OUTPUT_MAKEUP_GAIN: f32 = 4.0;

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
    /// Stable queue of the notes the player pressed, for the live matcher.
    played: Arc<ArrayQueue<PlayedNote>>,
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
        let played = Arc::new(ArrayQueue::new(COMMAND_CAPACITY));
        let audio_commands = Arc::clone(&commands);
        let audio_played = Arc::clone(&played);
        std::thread::Builder::new()
            .name("audio".to_string())
            .spawn(move || audio_loop(receiver, audio_commands, audio_played))
            .expect("the audio thread should start");
        AudioEngine {
            requests,
            commands,
            played,
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

    /// Drains the notes the player pressed, with the score positions they landed on.
    pub fn take_played(&self) -> Vec<PlayedNote> {
        let mut played = Vec::new();
        while let Some(note) = self.played.pop() {
            played.push(note);
        }
        played
    }

    /// Sounds the distinct cue for a wrong note: a short xylophone blip on its own channel,
    /// clearly apart from the piano the player is hearing. The instrument decays, so no
    /// note-off is needed.
    pub fn play_wrong_note(&self) {
        self.send(AudioCommand::NoteOn {
            channel: CUE_CHANNEL,
            pitch: CUE_PITCH,
            velocity: CUE_VELOCITY,
        });
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

    /// Replaces the notes the player must play for follow-you mode.
    pub fn set_gates(&self, gates: Arc<Vec<Gate>>) {
        self.control(PlaybackControl::SetGates(gates));
    }

    /// Turns follow-you mode on or off: the playhead waits at each gate until the note is played.
    pub fn set_follow(&self, follow: bool) {
        self.control(PlaybackControl::SetFollow(follow));
    }

    /// Shifts the piece up (positive) or down (negative) by up to 12 semitones.
    pub fn set_transpose(&self, semitones: i32) {
        self.control(PlaybackControl::SetTranspose(semitones));
    }

    /// Sets the volume of the piece's own parts, as a linear gain, 0.0 to 1.0.
    pub fn set_music_volume(&self, gain: f32) {
        self.control(PlaybackControl::SetMusicVolume(gain));
    }

    /// Sets the volume of the player's own notes, as a linear gain, 0.0 to 1.0.
    pub fn set_player_volume(&self, gain: f32) {
        self.control(PlaybackControl::SetPlayerVolume(gain));
    }

    /// Sets the master output volume, as a linear gain, 0.0 to 1.0, applied to the whole mix.
    pub fn set_output_volume(&self, gain: f32) {
        self.control(PlaybackControl::SetOutputVolume(gain));
    }

    /// Sets one track's channel volume, as a linear gain, 0.0 to 1.0.
    pub fn set_track_volume(&self, channel: u8, gain: f32) {
        self.control(PlaybackControl::SetTrackVolume { channel, gain });
    }

    /// Moves the playhead to `position_ms` in the current piece.
    pub fn seek(&self, position_ms: u64) {
        self.control(PlaybackControl::Seek(position_ms as f64));
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
fn audio_loop(
    requests: Receiver<AudioRequest>,
    commands: Arc<ArrayQueue<AudioCommand>>,
    played: Arc<ArrayQueue<PlayedNote>>,
) {
    let mut stream: Option<cpal::Stream> = None;
    while let Ok(request) = requests.recv() {
        // Drop the previous stream before opening the new one.
        drop(stream.take());
        match build_stream(&request, &commands, &played) {
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
    played: &Arc<ArrayQueue<PlayedNote>>,
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
        played: Arc::clone(played),
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
    /// Volume of the piece's own parts, as a linear gain.
    music_volume: f32,
    /// Volume of the player's notes, as a linear gain.
    player_volume: f32,
    /// Master output volume, as a linear gain, applied to the whole mix.
    output_volume: f32,
    /// Per-channel gain for the piece's tracks, on top of the music gain.
    track_volumes: [f32; 16],
    playing: bool,
    /// Whether the playhead waits for the player at each gate.
    follow: bool,
    gates: Arc<Vec<Gate>>,
    /// The next gate the player has to play.
    gate: usize,
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
            music_volume: 1.0,
            player_volume: 1.0,
            output_volume: 1.0,
            track_volumes: [1.0; 16],
            playing: false,
            follow: false,
            gates: Arc::new(Vec::new()),
            gate: 0,
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

    /// Score time the playhead may not pass in follow mode, or infinity. The player is allowed to
    /// fall `FOLLOW_TOLERANCE_MS` behind an unplayed gate before the accompaniment waits for it.
    fn gate_ms(&self) -> f64 {
        if self.follow {
            self.gates
                .get(self.gate)
                .map_or(f64::INFINITY, |gate| gate.start_ms + FOLLOW_TOLERANCE_MS)
        } else {
            f64::INFINITY
        }
    }

    /// Consumes the player's note, releasing the gate it belongs to.
    fn player_note(&mut self, pitch: u8) {
        if self.follow
            && let Some(gate) = self.gates.get(self.gate)
            && gate.pitch == pitch
            && self.position_ms + FOLLOW_LEAD_MS >= gate.start_ms
        {
            self.gate += 1;
        }
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
    played: Arc<ArrayQueue<PlayedNote>>,
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
            // Player notes are reported with the position they landed on: note-ons release gates
            // and are judged; both directions light the on-screen keyboard.
            match command {
                AudioCommand::NoteOn {
                    channel: PLAYER_CHANNEL,
                    pitch,
                    ..
                } => {
                    self.playback.player_note(pitch);
                    let _ = self.played.push(PlayedNote {
                        position_ms: self.playback.position_ms,
                        pitch,
                        down: true,
                    });
                }
                AudioCommand::NoteOff {
                    channel: PLAYER_CHANNEL,
                    pitch,
                } => {
                    let _ = self.played.push(PlayedNote {
                        position_ms: self.playback.position_ms,
                        pitch,
                        down: false,
                    });
                }
                _ => {}
            }
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
                self.playback.gate = 0;
                // A new SoundFont may be in place since the gains were last set.
                self.apply_mix();
            }
            PlaybackControl::SetGates(gates) => {
                self.playback.gates = gates;
                self.playback.gate = 0;
            }
            PlaybackControl::SetFollow(follow) => {
                self.playback.follow = follow;
            }
            PlaybackControl::SetTempoPercent(percent) => {
                self.playback.tempo_percent = percent.clamp(MIN_TEMPO_PERCENT, MAX_TEMPO_PERCENT);
            }
            PlaybackControl::SetTranspose(semitones) => {
                self.playback.transpose =
                    semitones.clamp(-TRANSPOSE_SEMITONES, TRANSPOSE_SEMITONES);
            }
            PlaybackControl::SetMusicVolume(gain) => {
                self.playback.music_volume = gain.clamp(0.0, 1.0);
                self.apply_mix();
            }
            PlaybackControl::SetPlayerVolume(gain) => {
                self.playback.player_volume = gain.clamp(0.0, 1.0);
                self.apply_mix();
            }
            PlaybackControl::SetOutputVolume(gain) => {
                self.playback.output_volume = gain.clamp(0.0, 1.0);
            }
            PlaybackControl::SetTrackVolume { channel, gain } => {
                if let Some(slot) = self.playback.track_volumes.get_mut(usize::from(channel)) {
                    *slot = gain.clamp(0.0, 1.0);
                    self.apply_mix();
                }
            }
            PlaybackControl::Seek(position_ms) => {
                let target = position_ms.max(0.0);
                // Skip every command before the target and silence anything still sounding.
                self.playback.index = self
                    .playback
                    .score
                    .partition_point(|event| event.at_ms < target);
                // Skip gates the playhead has already passed, so follow mode waits at the next one.
                self.playback.gate = self
                    .playback
                    .gates
                    .partition_point(|gate| gate.start_ms <= target);
                self.playback.position_ms = target;
                self.synth.apply(AudioCommand::AllNotesOff);
            }
            PlaybackControl::Start => self.playback.playing = true,
            PlaybackControl::Stop => {
                self.playback.playing = false;
                self.synth.apply(AudioCommand::AllNotesOff);
            }
        }
    }

    /// Pushes the mix to the synthesizer: the file's own parts take the Music gain as channel
    /// volume, the player's channel the My notes gain, and the cue channel stays at full so a
    /// mistake is heard even with the music down. Channel volume is the only per-source gain the
    /// SoundFont synth offers, and it rides sounding notes, so a slider move takes effect without
    /// restarting them. The cue's instrument is set here too, since a new SoundFont resets every
    /// channel.
    fn apply_mix(&mut self) {
        let music = self.playback.music_volume;
        for channel in 0..=15u8 {
            if channel == PLAYER_CHANNEL || channel == CUE_CHANNEL {
                continue;
            }
            let track = self.playback.track_volumes[usize::from(channel)];
            self.synth.apply(AudioCommand::ControlChange {
                channel,
                controller: VOLUME_CONTROLLER,
                value: cc7(music * track),
            });
        }
        self.synth.apply(AudioCommand::ControlChange {
            channel: PLAYER_CHANNEL,
            controller: VOLUME_CONTROLLER,
            value: cc7(self.playback.player_volume),
        });
        self.synth.apply(AudioCommand::ProgramChange {
            channel: CUE_CHANNEL,
            program: CUE_PROGRAM,
        });
        self.synth.apply(AudioCommand::ControlChange {
            channel: CUE_CHANNEL,
            controller: VOLUME_CONTROLLER,
            value: cc7(1.0),
        });
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
            // In follow mode the playhead may not pass the next gate. If it already sits on the
            // gate (or past a stale one), the limit is the current position and time stands still.
            let limit = self.playback.gate_ms().max(self.playback.position_ms);
            let end_ms = end_ms.min(limit);
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
        // rustysynth renders most SoundFonts well below full scale, so the mix takes a make-up
        // gain scaled by the Output slider, then a clamp so a loud moment distorts rather than
        // overflows.
        let gain = self.playback.output_volume * OUTPUT_MAKEUP_GAIN;
        {
            let left = &mut self.left[..count];
            let right = &mut self.right[..count];
            self.synth.render(left, right);
            for sample in left.iter_mut().chain(right.iter_mut()) {
                *sample = (*sample * gain).clamp(-1.0, 1.0);
            }
        }
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

/// A 0.0..=1.0 gain as a 7-bit MIDI channel-volume value.
fn cc7(gain: f32) -> u8 {
    (gain.clamp(0.0, 1.0) * 127.0).round() as u8
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
    use super::{
        MAX_BLOCK_FRAMES, OUTPUT_MAKEUP_GAIN, Renderer, ScorePlayback, cc7, shift, transposed,
        write_frames,
    };
    use crate::audio::Gate;
    use crate::audio::PLAYER_CHANNEL;
    use crate::audio::event::{AudioCommand, PlaybackControl, PlayedNote, ScheduledCommand};
    use crate::audio::synth::Synth;
    use crate::audio::{CUE_CHANNEL, CUE_PROGRAM};
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

    #[test]
    fn seek_skips_passed_events_and_resumes_at_the_target() {
        let mut renderer = renderer(
            1000,
            vec![
                scheduled(0.0, note_on(60)),
                scheduled(500.0, note_off(60)),
                scheduled(1000.0, note_on(62)),
            ],
        );
        renderer.push_control(PlaybackControl::Start);
        renderer.render(100);

        renderer.push_control(PlaybackControl::Seek(750.0));
        renderer.render(100);
        assert_eq!(renderer.published_ms(), 850);

        // The 0 and 500 commands were skipped; the playhead resumes at 750.
        renderer.render(200);
        assert_eq!(
            renderer.synth.events,
            vec![
                (0, note_on(60)),
                (100, AudioCommand::AllNotesOff),
                (350, note_on(62)),
            ]
        );
    }

    #[test]
    fn a_player_note_is_reported_with_its_position() {
        let mut renderer = renderer(1000, vec![]);
        renderer.push_control(PlaybackControl::Start);
        renderer
            .commands
            .push(player_note_on(60))
            .expect("the command queue should fit");

        renderer.render(10);

        assert_eq!(
            renderer.played.pop(),
            Some(PlayedNote {
                position_ms: 0.0,
                pitch: 60,
                down: true
            })
        );
    }

    #[test]
    fn a_note_on_another_channel_is_not_reported() {
        let mut renderer = renderer(1000, vec![]);
        renderer
            .commands
            .push(note_on(60))
            .expect("the command queue should fit");

        renderer.render(10);

        assert_eq!(renderer.played.pop(), None);
    }

    #[test]
    fn follow_only_waits_once_the_player_falls_behind() {
        let mut renderer = renderer(1000, vec![scheduled(500.0, note_on(60))]);
        renderer.push_control(PlaybackControl::SetGates(Arc::new(vec![gate(60, 500.0)])));
        renderer.push_control(PlaybackControl::SetFollow(true));
        renderer.push_control(PlaybackControl::Start);

        // The accompaniment keeps flowing within the tolerance, and the score note plays.
        renderer.render(700);
        assert_eq!(renderer.synth.events, vec![(500, note_on(60))]);
        assert_eq!(renderer.published_ms(), 700);

        // Past the gate plus the tolerance it waits, instead of running further ahead.
        renderer.render(1000);
        assert_eq!(renderer.published_ms(), 750);

        // Playing the note releases the gate and the playhead resumes.
        renderer
            .commands
            .push(player_note_on(60))
            .expect("the command queue should fit");
        renderer.render(10);
        assert_eq!(renderer.published_ms(), 760);
    }

    #[test]
    fn a_wrong_note_leaves_the_playhead_waiting() {
        let mut renderer = renderer(1000, vec![scheduled(500.0, note_on(60))]);
        renderer.push_control(PlaybackControl::SetGates(Arc::new(vec![gate(60, 500.0)])));
        renderer.push_control(PlaybackControl::SetFollow(true));
        renderer.push_control(PlaybackControl::Start);

        // Fall far enough behind to be waiting at the gate.
        renderer.render(1000);
        renderer
            .commands
            .push(player_note_on(62))
            .expect("the command queue should fit");
        renderer.render(200);

        // The player's wrong note sounds, but the accompaniment stays clamped.
        assert_eq!(
            renderer.synth.events.last(),
            Some(&(1000, player_note_on(62)))
        );
        assert_eq!(renderer.published_ms(), 750);
    }

    #[test]
    fn maps_gain_to_channel_volume() {
        assert_eq!(cc7(0.0), 0);
        assert_eq!(cc7(1.0), 127);
        assert_eq!(cc7(0.5), 64);
        assert_eq!(cc7(2.0), 127);
    }

    #[test]
    fn volume_changes_scale_each_source_on_its_own_channel() {
        let mut renderer = renderer(1000, vec![]);
        renderer.push_control(PlaybackControl::SetMusicVolume(0.5));
        renderer.push_control(PlaybackControl::SetPlayerVolume(0.25));
        renderer.render(1);

        let cc = |channel, value| AudioCommand::ControlChange {
            channel,
            controller: 7,
            value,
        };
        // The file's parts take the music gain; the player's channel takes its own.
        assert!(renderer.synth.events.contains(&(0, cc(0, 64))));
        assert!(renderer.synth.events.contains(&(0, cc(PLAYER_CHANNEL, 32))));
        assert!(!renderer.synth.events.contains(&(0, cc(PLAYER_CHANNEL, 64))));
        // The cue channel keeps its instrument and a full volume, whatever the music gain is.
        assert!(renderer.synth.events.contains(&(
            0,
            AudioCommand::ProgramChange {
                channel: CUE_CHANNEL,
                program: CUE_PROGRAM,
            }
        )));
        assert!(renderer.synth.events.contains(&(0, cc(CUE_CHANNEL, 127))));
        assert!(!renderer.synth.events.contains(&(0, cc(CUE_CHANNEL, 64))));
    }

    #[test]
    fn track_volume_scales_only_its_channel() {
        let mut renderer = renderer(1000, vec![]);
        renderer.push_control(PlaybackControl::SetTrackVolume {
            channel: 3,
            gain: 0.5,
        });
        renderer.render(1);

        let cc = |channel, value| AudioCommand::ControlChange {
            channel,
            controller: 7,
            value,
        };
        // Channel 3 takes its own gain; an untouched channel keeps the full music gain.
        assert!(renderer.synth.events.contains(&(0, cc(3, 64))));
        assert!(renderer.synth.events.contains(&(0, cc(0, 127))));
    }

    #[test]
    fn output_gain_scales_and_clamps_the_mix() {
        // A synth returning a constant, so the master stage is easy to read.
        struct ConstSynth;
        impl Synth for ConstSynth {
            fn apply(&mut self, _command: AudioCommand) {}
            fn render(&mut self, left: &mut [f32], right: &mut [f32]) {
                left.fill(0.5);
                right.fill(0.5);
            }
        }

        let mut renderer = Renderer {
            synth: ConstSynth,
            commands: Arc::new(ArrayQueue::new(16)),
            played: Arc::new(ArrayQueue::new(16)),
            control: Arc::new(ArrayQueue::new(16)),
            position: Arc::new(AtomicU64::new(0)),
            channels: 2,
            sample_rate: 1000,
            playback: ScorePlayback {
                output_volume: 0.25,
                ..ScorePlayback::default()
            },
            left: vec![0.0; MAX_BLOCK_FRAMES],
            right: vec![0.0; MAX_BLOCK_FRAMES],
        };

        // A quarter of the output gain scales the quiet sample, away from the rail.
        let mut data = vec![0.0; 8];
        renderer.render_block(&mut data);
        assert!((data[0] - 0.5 * 0.25 * OUTPUT_MAKEUP_GAIN).abs() < 1e-6);

        // Full output gain on the same sample clamps to the rail instead of overflowing.
        renderer.playback.output_volume = 1.0;
        renderer.render_block(&mut data);
        assert_eq!(data[0], 1.0);
    }

    #[test]
    fn follow_off_ignores_the_gates() {
        let mut renderer = renderer(1000, vec![scheduled(500.0, note_on(60))]);
        renderer.push_control(PlaybackControl::SetGates(Arc::new(vec![gate(60, 500.0)])));
        renderer.push_control(PlaybackControl::Start);

        renderer.render(1000);

        assert_eq!(renderer.synth.events, vec![(500, note_on(60))]);
        assert_eq!(renderer.published_ms(), 1000);
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
            played: Arc::new(ArrayQueue::new(16)),
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

    fn player_note_on(pitch: u8) -> AudioCommand {
        AudioCommand::NoteOn {
            channel: PLAYER_CHANNEL,
            pitch,
            velocity: 100,
        }
    }

    fn note_off(pitch: u8) -> AudioCommand {
        AudioCommand::NoteOff { channel: 0, pitch }
    }

    fn gate(pitch: u8, start_ms: f64) -> Gate {
        Gate { pitch, start_ms }
    }
}
