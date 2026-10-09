use std::collections::VecDeque;
use std::fs::File;
use std::io::BufWriter;
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

use chrono::Local;
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use cpal::{FromSample, Sample, SampleFormat, SizedSample};
use hound::{WavSpec, WavWriter};
use serde::Serialize;

use crate::error::{AppErrorDto, CmdResult};

/// Mixed output sample rate (mono PCM).
const TARGET_RATE: u32 = 48_000;

#[derive(Debug, Clone, Serialize)]
pub struct InputDeviceDto {
    pub id: String,
    pub name: String,
    pub is_default: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DevicesResponse {
    pub devices: Vec<InputDeviceDto>,
    pub default_id: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordStartResponse {
    pub path: String,
    pub device_name: String,
    pub output_device_name: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordStopResponse {
    pub path: String,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecordStatusResponse {
    pub state: String,
    pub path: Option<String>,
    pub started_at: Option<String>,
    pub device_name: Option<String>,
    pub output_device_name: Option<String>,
    /// Smoothed microphone amplitude in \[0, 1\].
    pub mic_level: f32,
    /// Smoothed system-loopback amplitude in \[0, 1\].
    pub system_level: f32,
}

struct RateConverter {
    src_rate: u32,
    pos: f64,
    pending: Vec<f32>,
}

impl RateConverter {
    fn new(src_rate: u32) -> Self {
        Self {
            src_rate: src_rate.max(1),
            pos: 0.0,
            pending: Vec::new(),
        }
    }

    fn push(&mut self, mono: &[f32], out: &mut VecDeque<f32>) {
        if mono.is_empty() {
            return;
        }
        // Common case: avoid interpolation drift when rates already match.
        if self.src_rate == TARGET_RATE {
            out.extend(mono.iter().copied());
            return;
        }
        self.pending.extend_from_slice(mono);
        let step = f64::from(self.src_rate) / f64::from(TARGET_RATE);
        while self.pos + 1.0 < self.pending.len() as f64 {
            let i0 = self.pos.floor() as usize;
            let frac = (self.pos - i0 as f64) as f32;
            let a = self.pending[i0];
            let b = self.pending[i0 + 1];
            out.push_back(a * (1.0 - frac) + b * frac);
            self.pos += step;
        }
        let drop = self.pos.floor() as usize;
        if drop > 0 {
            self.pending.drain(..drop.min(self.pending.len()));
            self.pos -= drop as f64;
            if self.pos < 0.0 {
                self.pos = 0.0;
            }
        }
    }
}

struct LevelMeter {
    /// Exponentially smoothed RMS in \[0, 1\].
    rms: f32,
}

impl LevelMeter {
    fn observe(&mut self, samples: &[f32]) {
        if samples.is_empty() {
            // Gentle decay when a callback has no new frames.
            self.rms *= 0.92;
            return;
        }
        let mut sum_sq = 0.0f32;
        let mut peak = 0.0f32;
        for &s in samples {
            let a = s.abs();
            if a > peak {
                peak = a;
            }
            sum_sq += s * s;
        }
        let rms = (sum_sq / samples.len() as f32).sqrt();
        // Blend peak slightly so short consonants register visually.
        let instant = (rms * 0.7 + peak * 0.3).clamp(0.0, 1.0);
        self.rms = self.rms * 0.72 + instant * 0.28;
    }
}

struct SharedMix {
    mic: VecDeque<f32>,
    loopback: VecDeque<f32>,
    writer: Option<WavWriter<BufWriter<File>>>,
    frames: u64,
    err: Option<String>,
    mic_meter: LevelMeter,
    system_meter: LevelMeter,
}

impl SharedMix {
    fn flush_paired(&mut self) {
        while !self.mic.is_empty() && !self.loopback.is_empty() {
            let a = self.mic.pop_front().unwrap_or(0.0);
            let b = self.loopback.pop_front().unwrap_or(0.0);
            if let Err(msg) = self.write_mixed(a, b) {
                self.err = Some(msg);
                return;
            }
        }
    }

    fn flush_remainder_with_silence(&mut self) {
        while !self.mic.is_empty() || !self.loopback.is_empty() {
            let a = self.mic.pop_front().unwrap_or(0.0);
            let b = self.loopback.pop_front().unwrap_or(0.0);
            if let Err(msg) = self.write_mixed(a, b) {
                self.err = Some(msg);
                return;
            }
        }
    }

    fn write_mixed(&mut self, a: f32, b: f32) -> Result<(), String> {
        let mixed = (a + b).clamp(-1.0, 1.0);
        let sample = (mixed * f32::from(i16::MAX)) as i16;
        if let Some(writer) = self.writer.as_mut() {
            writer
                .write_sample(sample)
                .map_err(|_| "Failed while writing mixed audio samples".to_string())?;
        }
        self.frames = self.frames.saturating_add(1);
        Ok(())
    }
}

struct ActiveRecording {
    /// Final M4A path returned to the frontend after stop.
    path: PathBuf,
    /// Temporary WAV written during capture (converted on stop).
    wav_path: PathBuf,
    started_at: Instant,
    started_at_iso: String,
    device_name: String,
    output_device_name: String,
    mix: Arc<Mutex<SharedMix>>,
    _mic_stream: cpal::Stream,
    _loop_stream: cpal::Stream,
}

enum WorkerRequest {
    Start {
        recording_dir_stored: String,
        device_id: Option<String>,
        reply: Sender<CmdResult<RecordStartResponse>>,
    },
    Stop {
        reply: Sender<CmdResult<RecordStopResponse>>,
    },
    Status {
        reply: Sender<RecordStatusResponse>,
    },
}

/// Sendable handle to a dedicated audio worker thread that owns `cpal::Stream`
/// (`Stream` is `!Send` on some platforms, so it cannot live in Tauri `AppState`).
pub struct RecordingSession {
    tx: Mutex<Sender<WorkerRequest>>,
}

impl Default for RecordingSession {
    fn default() -> Self {
        Self::spawn()
    }
}

impl RecordingSession {
    pub fn spawn() -> Self {
        let (tx, rx) = mpsc::channel::<WorkerRequest>();
        thread::Builder::new()
            .name("meetphant-recording".into())
            .spawn(move || worker_loop(rx))
            .expect("failed to spawn recording worker thread");
        Self { tx: Mutex::new(tx) }
    }

    fn send(&self, req: WorkerRequest) -> CmdResult<()> {
        self.tx
            .lock()
            .map_err(|_| AppErrorDto::internal("Recording lock poisoned"))?
            .send(req)
            .map_err(|_| AppErrorDto::internal("Recording worker is not running"))
    }

    pub fn start(
        &self,
        recording_dir_stored: &str,
        device_id: Option<&str>,
    ) -> CmdResult<RecordStartResponse> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(WorkerRequest::Start {
            recording_dir_stored: recording_dir_stored.to_string(),
            device_id: device_id.map(|s| s.to_string()),
            reply: reply_tx,
        })?;
        reply_rx
            .recv()
            .map_err(|_| AppErrorDto::internal("Recording worker did not respond"))?
    }

    pub fn stop(&self) -> CmdResult<RecordStopResponse> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(WorkerRequest::Stop { reply: reply_tx })?;
        reply_rx
            .recv()
            .map_err(|_| AppErrorDto::internal("Recording worker did not respond"))?
    }

    pub fn status(&self) -> CmdResult<RecordStatusResponse> {
        let (reply_tx, reply_rx) = mpsc::channel();
        self.send(WorkerRequest::Status { reply: reply_tx })?;
        reply_rx
            .recv()
            .map_err(|_| AppErrorDto::internal("Recording worker did not respond"))
    }
}

fn worker_loop(rx: Receiver<WorkerRequest>) {
    let mut active: Option<ActiveRecording> = None;
    while let Ok(req) = rx.recv() {
        match req {
            WorkerRequest::Start {
                recording_dir_stored,
                device_id,
                reply,
            } => {
                let result = if active.is_some() {
                    Err(AppErrorDto::record_busy())
                } else {
                    match start_recording_inner(&recording_dir_stored, device_id.as_deref()) {
                        Ok(started) => {
                            let response = RecordStartResponse {
                                path: started.path.to_string_lossy().to_string(),
                                device_name: started.device_name.clone(),
                                output_device_name: started.output_device_name.clone(),
                            };
                            active = Some(started);
                            Ok(response)
                        }
                        Err(err) => Err(err),
                    }
                };
                let _ = reply.send(result);
            }
            WorkerRequest::Stop { reply } => {
                let result = match active.take() {
                    Some(rec) => finalize_recording(rec),
                    None => Err(AppErrorDto::record_not_active()),
                };
                let _ = reply.send(result);
            }
            WorkerRequest::Status { reply } => {
                let status = match &active {
                    Some(rec) => {
                        let (mic_level, system_level) = rec
                            .mix
                            .lock()
                            .map(|g| (g.mic_meter.rms, g.system_meter.rms))
                            .unwrap_or((0.0, 0.0));
                        RecordStatusResponse {
                            state: "recording".into(),
                            path: Some(rec.path.to_string_lossy().to_string()),
                            started_at: Some(rec.started_at_iso.clone()),
                            device_name: Some(rec.device_name.clone()),
                            output_device_name: Some(rec.output_device_name.clone()),
                            mic_level,
                            system_level,
                        }
                    }
                    None => RecordStatusResponse {
                        state: "idle".into(),
                        path: None,
                        started_at: None,
                        device_name: None,
                        output_device_name: None,
                        mic_level: 0.0,
                        system_level: 0.0,
                    },
                };
                let _ = reply.send(status);
            }
        }
    }
}

pub fn default_recording_dir() -> CmdResult<PathBuf> {
    let docs = dirs::document_dir()
        .ok_or_else(|| AppErrorDto::io_error("Could not resolve the user Documents folder"))?;
    Ok(docs.join("Meetphant").join("Recordings"))
}

pub fn resolve_recording_dir(stored: &str) -> CmdResult<PathBuf> {
    let trimmed = stored.trim();
    if trimmed.is_empty() {
        default_recording_dir()
    } else {
        Ok(PathBuf::from(trimmed))
    }
}

/// Validate a user-provided recording directory override.
/// Empty string means "use default" and is always valid.
pub fn validate_recording_dir_override(raw: &str) -> CmdResult<String> {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Ok(String::new());
    }

    let path = PathBuf::from(trimmed);
    if !path.is_absolute() {
        return Err(AppErrorDto::settings_invalid(
            "Recording directory must be an absolute path",
        ));
    }

    if path.exists() && !path.is_dir() {
        return Err(AppErrorDto::settings_invalid(
            "Recording directory path exists but is not a folder",
        ));
    }

    ensure_dir_writable(&path)?;
    Ok(trimmed.to_string())
}

pub fn ensure_dir_writable(dir: &Path) -> CmdResult<()> {
    std::fs::create_dir_all(dir)
        .map_err(|_| AppErrorDto::io_error("Could not create the recording directory"))?;

    let probe = dir.join(".meetphant-write-probe");
    match std::fs::write(&probe, b"ok") {
        Ok(()) => {
            let _ = std::fs::remove_file(&probe);
            Ok(())
        }
        Err(_) => Err(AppErrorDto::io_error("Recording directory is not writable")),
    }
}

pub fn list_input_devices() -> CmdResult<DevicesResponse> {
    let host = cpal::default_host();
    let default_name = host.default_input_device().and_then(|d| d.name().ok());

    let devices_iter = host.input_devices().map_err(|e| {
        AppErrorDto::record_device_error(format!("Failed to enumerate input devices: {e}"))
    })?;

    let mut devices = Vec::new();
    let mut default_id = None;

    for (index, device) in devices_iter.enumerate() {
        let name = device
            .name()
            .unwrap_or_else(|_| format!("Input device {index}"));
        let id = format!("{index}");
        let is_default = default_name.as_ref().is_some_and(|n| n == &name);
        if is_default {
            default_id = Some(id.clone());
        }
        devices.push(InputDeviceDto {
            id,
            name,
            is_default,
        });
    }

    if default_id.is_none() {
        if let Some(first) = devices.first() {
            default_id = Some(first.id.clone());
        }
    }

    if devices.is_empty() {
        return Err(AppErrorDto::record_no_device());
    }

    Ok(DevicesResponse {
        devices,
        default_id,
    })
}

fn find_input_device(device_id: Option<&str>) -> CmdResult<(cpal::Device, String)> {
    let host = cpal::default_host();
    let listed = list_input_devices()?;

    let target_id = match device_id {
        Some(id) if !id.trim().is_empty() => id.trim().to_string(),
        _ => listed
            .default_id
            .clone()
            .ok_or_else(AppErrorDto::record_no_device)?,
    };

    let meta = listed
        .devices
        .iter()
        .find(|d| d.id == target_id)
        .ok_or_else(|| {
            AppErrorDto::record_device_error("Selected audio input device was not found")
        })?;

    let index: usize = meta
        .id
        .parse()
        .map_err(|_| AppErrorDto::record_device_error("Invalid audio input device id"))?;

    let device = host
        .input_devices()
        .map_err(|e| {
            AppErrorDto::record_device_error(format!("Failed to open input devices: {e}"))
        })?
        .nth(index)
        .ok_or_else(|| {
            AppErrorDto::record_device_error("Selected audio input device was not found")
        })?;

    Ok((device, meta.name.clone()))
}

fn find_default_output_device() -> CmdResult<(cpal::Device, String)> {
    let host = cpal::default_host();
    let device = host.default_output_device().ok_or_else(|| {
        AppErrorDto::record_device_error(
            "No system playback device is available for loopback capture",
        )
    })?;
    let name = device
        .name()
        .unwrap_or_else(|_| "System output".to_string());
    Ok((device, name))
}

fn build_output_paths(dir: &Path) -> (PathBuf, PathBuf) {
    let stamp = Local::now().format("%Y%m%d-%H%M%S");
    let stem = format!("Meetphant-{stamp}");
    (
        dir.join(format!("{stem}.m4a")),
        dir.join(format!("{stem}.wav.partial")),
    )
}

fn ensure_ffmpeg_ready() -> bool {
    crate::services::ffmpeg_service::is_ready()
}

/// Best-effort background install so the next stop can encode to M4A without waiting.
pub fn prefetch_ffmpeg_in_background() {
    crate::services::ffmpeg_service::prefetch_in_background();
}

/// Convert a PCM WAV capture to AAC-in-M4A when FFmpeg is already available.
/// Returns `Ok(true)` if `m4a` was written, `Ok(false)` if caller should keep WAV.
fn try_encode_wav_to_m4a(wav: &Path, m4a: &Path) -> CmdResult<bool> {
    if !ensure_ffmpeg_ready() {
        return Ok(false);
    }

    let wav_s = wav.to_string_lossy().to_string();
    let m4a_s = m4a.to_string_lossy().to_string();

    let ffmpeg_bin = crate::services::ffmpeg_service::resolve_ffmpeg_path()
        .ok_or_else(|| AppErrorDto::io_error("FFmpeg is not available"))?;
    let mut child = ffmpeg_sidecar::command::FfmpegCommand::new_with_path(&ffmpeg_bin)
        .create_no_window()
        .overwrite()
        .input(&wav_s)
        .codec_audio("aac")
        .args(["-b:a", "128k", "-ac", "1", "-movflags", "+faststart"])
        .output(&m4a_s)
        .spawn()
        .map_err(|e| AppErrorDto::io_error(format!("Failed to start M4A encoder: {e}")))?;

    let mut last_error: Option<String> = None;
    match child.iter() {
        Ok(iter) => {
            for event in iter {
                if let ffmpeg_sidecar::event::FfmpegEvent::Error(msg) = event {
                    last_error = Some(msg);
                }
            }
        }
        Err(e) => {
            let _ = child.kill();
            return Err(AppErrorDto::io_error(format!(
                "M4A encoder I/O failed: {e}"
            )));
        }
    }

    let status = child
        .wait()
        .map_err(|e| AppErrorDto::io_error(format!("Failed while waiting for M4A encoder: {e}")))?;
    if !status.success() {
        let _ = std::fs::remove_file(m4a);
        let detail = last_error.unwrap_or_else(|| format!("exit status {status}"));
        return Err(AppErrorDto::io_error(format!(
            "M4A encoding failed: {detail}"
        )));
    }
    if !m4a.is_file() {
        return Err(AppErrorDto::io_error(
            "M4A encoding finished but the output file is missing",
        ));
    }
    Ok(true)
}

fn downmix_to_mono<T>(data: &[T], channels: u16) -> Vec<f32>
where
    T: Sample,
    f32: FromSample<T>,
{
    if channels == 0 || data.is_empty() {
        return Vec::new();
    }
    let ch = usize::from(channels);
    let frames = data.len() / ch;
    let mut out = Vec::with_capacity(frames);
    for frame in 0..frames {
        let mut sum = 0.0f32;
        for c in 0..ch {
            sum += f32::from_sample(data[frame * ch + c]);
        }
        out.push(sum / ch as f32);
    }
    out
}

fn start_recording_inner(
    recording_dir_stored: &str,
    device_id: Option<&str>,
) -> CmdResult<ActiveRecording> {
    #[cfg(not(windows))]
    {
        let _ = (recording_dir_stored, device_id);
        return Err(AppErrorDto::record_device_error(
            "Microphone + system audio mix is only supported on Windows in this version",
        ));
    }

    #[cfg(windows)]
    {
        start_recording_windows(recording_dir_stored, device_id)
    }
}

#[cfg(windows)]
fn start_recording_windows(
    recording_dir_stored: &str,
    device_id: Option<&str>,
) -> CmdResult<ActiveRecording> {
    // Download FFmpeg while the user is still recording (if needed).
    prefetch_ffmpeg_in_background();

    let dir = resolve_recording_dir(recording_dir_stored)?;
    ensure_dir_writable(&dir)?;

    let (mic_device, device_name) = find_input_device(device_id)?;
    let mic_config = mic_device.default_input_config().map_err(|e| {
        AppErrorDto::record_device_error(format!("Unsupported microphone configuration: {e}"))
    })?;

    let (out_device, output_device_name) = find_default_output_device()?;
    let loop_config = out_device.default_output_config().map_err(|e| {
        AppErrorDto::record_device_error(format!(
            "Unsupported system playback configuration for loopback: {e}"
        ))
    })?;

    let (m4a_path, wav_path) = build_output_paths(&dir);
    let spec = WavSpec {
        channels: 1,
        sample_rate: TARGET_RATE,
        bits_per_sample: 16,
        sample_format: hound::SampleFormat::Int,
    };

    let file = File::create(&wav_path)
        .map_err(|_| AppErrorDto::io_error("Could not create the temporary recording file"))?;
    let writer = WavWriter::new(BufWriter::new(file), spec)
        .map_err(|_| AppErrorDto::io_error("Could not initialize the WAV writer"))?;

    let mix = Arc::new(Mutex::new(SharedMix {
        mic: VecDeque::new(),
        loopback: VecDeque::new(),
        writer: Some(writer),
        frames: 0,
        err: None,
        mic_meter: LevelMeter { rms: 0.0 },
        system_meter: LevelMeter { rms: 0.0 },
    }));

    let mic_stream = build_capture_stream(
        &mic_device,
        &mic_config.clone().into(),
        mic_config.sample_format(),
        mic_config.channels(),
        mic_config.sample_rate().0,
        Arc::clone(&mix),
        CaptureSide::Mic,
    )?;

    let loop_stream = build_capture_stream(
        &out_device,
        &loop_config.clone().into(),
        loop_config.sample_format(),
        loop_config.channels(),
        loop_config.sample_rate().0,
        Arc::clone(&mix),
        CaptureSide::Loopback,
    )?;

    mic_stream.play().map_err(|e| {
        let _ = std::fs::remove_file(&wav_path);
        AppErrorDto::record_device_error(format!("Failed to start microphone stream: {e}"))
    })?;
    if let Err(e) = loop_stream.play() {
        drop(mic_stream);
        let _ = std::fs::remove_file(&wav_path);
        return Err(AppErrorDto::record_device_error(format!(
            "Failed to start system audio loopback: {e}"
        )));
    }

    if let Ok(guard) = mix.lock() {
        if let Some(msg) = guard.err.clone() {
            drop(mic_stream);
            drop(loop_stream);
            let _ = std::fs::remove_file(&wav_path);
            return Err(AppErrorDto::record_device_error(msg));
        }
    }

    Ok(ActiveRecording {
        path: m4a_path,
        wav_path,
        started_at: Instant::now(),
        started_at_iso: Local::now().to_rfc3339(),
        device_name,
        output_device_name,
        mix,
        _mic_stream: mic_stream,
        _loop_stream: loop_stream,
    })
}

#[derive(Clone, Copy)]
enum CaptureSide {
    Mic,
    Loopback,
}

#[cfg(windows)]
fn build_capture_stream(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    sample_format: SampleFormat,
    channels: u16,
    sample_rate: u32,
    mix: Arc<Mutex<SharedMix>>,
    side: CaptureSide,
) -> CmdResult<cpal::Stream> {
    match sample_format {
        SampleFormat::F32 => {
            build_capture_stream_typed::<f32>(device, config, channels, sample_rate, mix, side)
        }
        SampleFormat::I16 => {
            build_capture_stream_typed::<i16>(device, config, channels, sample_rate, mix, side)
        }
        SampleFormat::U16 => {
            build_capture_stream_typed::<u16>(device, config, channels, sample_rate, mix, side)
        }
        other => Err(AppErrorDto::record_device_error(format!(
            "Unsupported sample format: {other:?}"
        ))),
    }
}

#[cfg(windows)]
fn build_capture_stream_typed<T>(
    device: &cpal::Device,
    config: &cpal::StreamConfig,
    channels: u16,
    sample_rate: u32,
    mix: Arc<Mutex<SharedMix>>,
    side: CaptureSide,
) -> CmdResult<cpal::Stream>
where
    T: Sample + SizedSample + Send + 'static,
    f32: FromSample<T>,
{
    let mut converter = RateConverter::new(sample_rate);
    let mix_err = Arc::clone(&mix);

    device
        .build_input_stream(
            config,
            move |data: &[T], _| {
                let mono = downmix_to_mono(data, channels);
                let Ok(mut guard) = mix.lock() else {
                    return;
                };
                if guard.err.is_some() {
                    return;
                }
                match side {
                    CaptureSide::Mic => guard.mic_meter.observe(&mono),
                    CaptureSide::Loopback => guard.system_meter.observe(&mono),
                }
                let mut converted = VecDeque::new();
                converter.push(&mono, &mut converted);
                match side {
                    CaptureSide::Mic => guard.mic.extend(converted),
                    CaptureSide::Loopback => guard.loopback.extend(converted),
                }
                guard.flush_paired();
            },
            move |err| {
                if let Ok(mut guard) = mix_err.lock() {
                    guard.err = Some(format!("Audio stream error: {err}"));
                }
            },
            None,
        )
        .map_err(|e| {
            AppErrorDto::record_device_error(format!("Failed to open audio capture stream: {e}"))
        })
}

fn finalize_recording(active: ActiveRecording) -> CmdResult<RecordStopResponse> {
    let ActiveRecording {
        path: m4a_path,
        wav_path,
        started_at,
        mix,
        _mic_stream,
        _loop_stream,
        ..
    } = active;
    drop(_mic_stream);
    drop(_loop_stream);

    std::thread::sleep(Duration::from_millis(40));

    let frames = {
        let mut guard = mix
            .lock()
            .map_err(|_| AppErrorDto::internal("Recording mix lock poisoned"))?;
        if let Some(msg) = guard.err.clone() {
            drop(guard.writer.take());
            let _ = std::fs::remove_file(&wav_path);
            return Err(AppErrorDto::record_device_error(msg));
        }
        guard.flush_remainder_with_silence();
        if let Some(writer) = guard.writer.take() {
            writer
                .finalize()
                .map_err(|_| AppErrorDto::io_error("Could not finalize the temporary recording"))?;
        }
        guard.frames
    };

    if frames == 0 {
        let _ = std::fs::remove_file(&wav_path);
        return Err(AppErrorDto::invalid_argument(
            "Recording was empty; nothing was captured",
        ));
    }

    // Prefer M4A when FFmpeg is already present. Never block stop on a multi‑minute
    // first-time download (gyan.dev essentials zip ~80–100 MiB); Doubao accepts WAV too.
    let final_path = match try_encode_wav_to_m4a(&wav_path, &m4a_path) {
        Ok(true) => {
            let _ = std::fs::remove_file(&wav_path);
            m4a_path
        }
        Ok(false) => {
            prefetch_ffmpeg_in_background();
            let wav_final = m4a_path.with_extension("wav");
            std::fs::rename(&wav_path, &wav_final)
                .map_err(|_| AppErrorDto::io_error("Could not save the recording as WAV"))?;
            wav_final
        }
        Err(err) => {
            // Encoding failed but PCM may still be usable — fall back to WAV.
            let _ = std::fs::remove_file(&m4a_path);
            let wav_final = m4a_path.with_extension("wav");
            if std::fs::rename(&wav_path, &wav_final).is_ok() {
                wav_final
            } else {
                let _ = std::fs::remove_file(&wav_path);
                return Err(err);
            }
        }
    };

    let duration_ms = started_at.elapsed().as_millis() as u64;
    Ok(RecordStopResponse {
        path: final_path.to_string_lossy().to_string(),
        duration_ms,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    #[test]
    fn resolve_empty_uses_documents_meetphant_recordings() {
        let path = resolve_recording_dir("").expect("resolve");
        let path_str = path.to_string_lossy().replace('\\', "/");
        assert!(
            path_str.ends_with("Meetphant/Recordings"),
            "unexpected default path: {path_str}"
        );
    }

    #[test]
    fn resolve_override_keeps_absolute_path() {
        let override_path = if cfg!(windows) {
            r"D:\tmp\meetphant-recs"
        } else {
            "/tmp/meetphant-recs"
        };
        let path = resolve_recording_dir(override_path).expect("resolve");
        assert_eq!(path, PathBuf::from(override_path));
    }

    #[test]
    fn validate_relative_path_rejects() {
        let err = validate_recording_dir_override("relative/path").expect_err("relative");
        assert_eq!(err.code, "SETTINGS_INVALID");
    }

    #[test]
    fn validate_empty_resets_to_default() {
        let v = validate_recording_dir_override("  ").expect("empty");
        assert_eq!(v, "");
    }

    #[test]
    fn validate_writable_absolute_dir() {
        let base = std::env::temp_dir().join(format!("meetphant-rec-test-{}", std::process::id()));
        let _ = fs::remove_dir_all(&base);
        let validated = validate_recording_dir_override(&base.to_string_lossy()).expect("validate");
        assert_eq!(validated, base.to_string_lossy());
        assert!(base.is_dir());
        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn stop_without_start_is_not_active() {
        let session = RecordingSession::spawn();
        let err = session.stop().expect_err("not active");
        assert_eq!(err.code, "RECORD_NOT_ACTIVE");
    }

    #[test]
    fn status_idle_by_default() {
        let session = RecordingSession::spawn();
        let s = session.status().expect("status");
        assert_eq!(s.state, "idle");
        assert!(s.path.is_none());
        assert!(s.output_device_name.is_none());
        assert_eq!(s.mic_level, 0.0);
        assert_eq!(s.system_level, 0.0);
    }

    #[test]
    fn rate_converter_same_rate_preserves_samples() {
        let mut conv = RateConverter::new(TARGET_RATE);
        let mut out = VecDeque::new();
        conv.push(&[0.0, 0.5, -0.5, 1.0], &mut out);
        assert_eq!(out.len(), 4);
        assert!((out[0] - 0.0).abs() < 1e-5);
        assert!((out[1] - 0.5).abs() < 1e-5);
        assert!((out[3] - 1.0).abs() < 1e-5);
    }

    #[test]
    fn mix_sums_and_clamps() {
        let mut mix = SharedMix {
            mic: VecDeque::from([0.8, 0.9]),
            loopback: VecDeque::from([0.8, 0.2]),
            writer: None,
            frames: 0,
            err: None,
            mic_meter: LevelMeter { rms: 0.0 },
            system_meter: LevelMeter { rms: 0.0 },
        };
        mix.flush_paired();
        assert_eq!(mix.frames, 2);
        assert!(mix.mic.is_empty());
        assert!(mix.loopback.is_empty());
    }

    #[test]
    fn level_meter_rises_with_signal() {
        let mut meter = LevelMeter { rms: 0.0 };
        meter.observe(&[0.0; 64]);
        let quiet = meter.rms;
        meter.observe(&[0.6; 64]);
        assert!(meter.rms > quiet);
        assert!(meter.rms <= 1.0);
    }

    #[test]
    fn downmix_stereo_averages_channels() {
        let data: [f32; 4] = [1.0, -1.0, 0.5, 0.5];
        let mono = downmix_to_mono(&data, 2);
        assert_eq!(mono.len(), 2);
        assert!((mono[0] - 0.0).abs() < 1e-6);
        assert!((mono[1] - 0.5).abs() < 1e-6);
    }

    #[test]
    fn output_paths_use_m4a_final_and_wav_partial() {
        let dir = PathBuf::from(if cfg!(windows) { r"D:\tmp" } else { "/tmp" });
        let (m4a, wav) = build_output_paths(&dir);
        assert!(m4a
            .extension()
            .is_some_and(|e| e.eq_ignore_ascii_case("m4a")));
        assert!(wav.to_string_lossy().ends_with(".wav.partial"));
    }
}
