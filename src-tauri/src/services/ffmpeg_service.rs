use std::fs::{self, File};
use std::io::{copy, Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::thread;

use serde::Serialize;
use tauri::{AppHandle, Emitter};

use crate::error::{AppErrorDto, CmdResult};

static DOWNLOAD_IN_FLIGHT: AtomicBool = AtomicBool::new(false);
static INSTALL_DIR: OnceLock<PathBuf> = OnceLock::new();

fn progress_slot() -> &'static Mutex<FfmpegProgress> {
    static CELL: OnceLock<Mutex<FfmpegProgress>> = OnceLock::new();
    CELL.get_or_init(|| Mutex::new(FfmpegProgress::default()))
}

#[derive(Debug, Clone, Default)]
struct FfmpegProgress {
    phase: String,
    downloaded_bytes: u64,
    total_bytes: u64,
    message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct FfmpegStatus {
    pub installed: bool,
    pub busy: bool,
    /// `ready` | `missing` | `starting` | `downloading` | `unpacking` | `error`
    pub phase: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub path: Option<String>,
    pub message: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "snake_case")]
pub struct FfmpegProgressEvent {
    pub phase: String,
    pub downloaded_bytes: u64,
    pub total_bytes: u64,
    pub installed: bool,
    pub message: Option<String>,
}

/// Must be called once during app setup with a user-writable directory
/// (e.g. `app_data_dir/ffmpeg`). MSI installs under Program Files cannot
/// receive downloads next to the executable.
pub fn init_install_dir(dir: PathBuf) {
    let _ = INSTALL_DIR.set(dir);
}

fn install_dir() -> CmdResult<PathBuf> {
    INSTALL_DIR
        .get()
        .cloned()
        .ok_or_else(|| AppErrorDto::internal("FFmpeg install directory was not initialized"))
}

fn managed_binary_path() -> Option<PathBuf> {
    INSTALL_DIR.get().map(|dir| {
        let mut path = dir.join("ffmpeg");
        if cfg!(windows) {
            path.set_extension("exe");
        }
        path
    })
}

fn binary_runs(path: &Path) -> bool {
    let mut cmd = Command::new(path);
    cmd.arg("-version")
        .stderr(Stdio::null())
        .stdout(Stdio::null());
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd.status().map(|s| s.success()).unwrap_or(false)
}

/// Prefer managed (app data) download, then installer-bundled sidecar, then PATH.
pub fn resolve_ffmpeg_path() -> Option<PathBuf> {
    if let Some(path) = managed_binary_path().filter(|p| p.exists() && binary_runs(p)) {
        return Some(path);
    }
    if let Some(path) = bundled_binary_path().filter(|p| binary_runs(p)) {
        return Some(path);
    }
    let fallback = ffmpeg_sidecar::paths::ffmpeg_path();
    if binary_runs(&fallback) {
        Some(fallback)
    } else {
        None
    }
}

/// Offline NSIS builds ship FFmpeg via Tauri `externalBin` next to the app exe.
/// Production strips the target triple (`ffmpeg.exe`); `tauri dev` keeps it
/// (`ffmpeg-<triple>.exe`).
fn bundled_binary_path() -> Option<PathBuf> {
    let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
    bundled_candidates_in(&dir).into_iter().find(|p| p.exists())
}

fn bundled_candidates_in(dir: &Path) -> Vec<PathBuf> {
    let mut candidates: Vec<PathBuf> = Vec::new();

    let mut production = dir.join("ffmpeg");
    if cfg!(windows) {
        production.set_extension("exe");
    }
    candidates.push(production);

    if let Ok(triple) = std::env::var("TAURI_ENV_TARGET_TRIPLE") {
        if !triple.is_empty() {
            let mut named = dir.join(format!("ffmpeg-{triple}"));
            if cfg!(windows) {
                named.set_extension("exe");
            }
            candidates.push(named);
        }
    }

    #[cfg(windows)]
    {
        candidates.push(dir.join("ffmpeg-x86_64-pc-windows-msvc.exe"));
        candidates.push(dir.join("ffmpeg-aarch64-pc-windows-msvc.exe"));
    }

    candidates
}

pub fn is_ready() -> bool {
    resolve_ffmpeg_path().is_some()
}

fn path_string() -> Option<String> {
    resolve_ffmpeg_path().map(|p| p.to_string_lossy().to_string())
}

fn read_progress() -> FfmpegProgress {
    progress_slot()
        .lock()
        .map(|g| g.clone())
        .unwrap_or_default()
}

fn write_progress(update: impl FnOnce(&mut FfmpegProgress)) {
    if let Ok(mut g) = progress_slot().lock() {
        update(&mut g);
    }
}

pub fn status() -> FfmpegStatus {
    let installed = is_ready();
    let busy = DOWNLOAD_IN_FLIGHT.load(Ordering::SeqCst);
    let progress = read_progress();

    let phase = if busy {
        if progress.phase.is_empty() {
            "starting".into()
        } else {
            progress.phase.clone()
        }
    } else if installed {
        "ready".into()
    } else if progress.phase == "error" {
        "error".into()
    } else {
        "missing".into()
    };

    FfmpegStatus {
        installed,
        busy,
        phase,
        downloaded_bytes: progress.downloaded_bytes,
        total_bytes: progress.total_bytes,
        path: path_string(),
        message: progress.message.clone(),
    }
}

pub fn prefetch_in_background() {
    if is_ready() || DOWNLOAD_IN_FLIGHT.load(Ordering::SeqCst) {
        return;
    }
    let _ = start_download(None);
}

pub fn start_download(app: Option<AppHandle>) -> CmdResult<FfmpegStatus> {
    if is_ready() {
        write_progress(|p| {
            p.phase = "ready".into();
            p.message = Some("FFmpeg is ready".into());
        });
        return Ok(status());
    }

    if DOWNLOAD_IN_FLIGHT
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(status());
    }

    write_progress(|p| {
        p.phase = "starting".into();
        p.downloaded_bytes = 0;
        p.total_bytes = 0;
        p.message = Some("Preparing FFmpeg download…".into());
    });

    let app_for_thread = app.clone();
    thread::Builder::new()
        .name("meetphant-ffmpeg-download".into())
        .spawn(move || {
            emit_progress(&app_for_thread);
            let result = download_and_install(|phase, downloaded, total, message| {
                write_progress(|p| {
                    p.phase = phase.into();
                    p.downloaded_bytes = downloaded;
                    p.total_bytes = total;
                    p.message = Some(message.into());
                });
                emit_progress(&app_for_thread);
            });

            match result {
                Ok(()) => {
                    write_progress(|p| {
                        p.phase = "ready".into();
                        p.message = Some("FFmpeg is ready".into());
                    });
                }
                Err(e) => {
                    tracing::warn!(code = %e.code, "ffmpeg.download failed");
                    write_progress(|p| {
                        p.phase = "error".into();
                        p.message = Some(e.message.clone());
                    });
                }
            }

            DOWNLOAD_IN_FLIGHT.store(false, Ordering::SeqCst);
            emit_progress(&app_for_thread);
        })
        .map_err(|e| {
            DOWNLOAD_IN_FLIGHT.store(false, Ordering::SeqCst);
            AppErrorDto::internal(format!("Failed to start FFmpeg download: {e}"))
        })?;

    Ok(status())
}

fn emit_progress(app: &Option<AppHandle>) {
    let Some(app) = app else {
        return;
    };
    let s = status();
    let payload = FfmpegProgressEvent {
        phase: s.phase,
        downloaded_bytes: s.downloaded_bytes,
        total_bytes: s.total_bytes,
        installed: s.installed,
        message: s.message,
    };
    let _ = app.emit("ffmpeg-progress", payload);
}

type ProgressCb<'a> = dyn Fn(&str, u64, u64, &str) + 'a;

fn download_and_install(on_progress: impl Fn(&str, u64, u64, &str)) -> CmdResult<()> {
    #[cfg(not(windows))]
    {
        let _ = on_progress;
        return Err(AppErrorDto::io_error(
            "Automatic FFmpeg download is only supported on Windows in this version",
        ));
    }

    #[cfg(windows)]
    {
        download_and_install_windows(&on_progress as &ProgressCb<'_>)
    }
}

#[cfg(windows)]
fn download_and_install_windows(on_progress: &ProgressCb<'_>) -> CmdResult<()> {
    // Keep in sync with scripts/ffmpeg-pin.json (offline installer uses the same pin).
    const URL: &str =
        "https://github.com/GyanD/codexffmpeg/releases/download/8.1/ffmpeg-8.1-essentials_build.zip";

    let dest_dir = install_dir()?;
    fs::create_dir_all(&dest_dir).map_err(|e| {
        AppErrorDto::io_error(format!("Could not create FFmpeg install directory: {e}"))
    })?;

    let archive_path = dest_dir.join("ffmpeg-release-essentials.zip");
    on_progress("starting", 0, 0, "Starting download…");

    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(600))
        .build()
        .map_err(|_| AppErrorDto::io_error("Could not create HTTP client for FFmpeg download"))?;

    let mut response = client
        .get(URL)
        .send()
        .map_err(|e| AppErrorDto::io_error(format!("FFmpeg download request failed: {e}")))?;
    if !response.status().is_success() {
        return Err(AppErrorDto::io_error(format!(
            "FFmpeg download HTTP {}",
            response.status()
        )));
    }

    let total = response.content_length().unwrap_or(0);
    let mut file = File::create(&archive_path)
        .map_err(|e| AppErrorDto::io_error(format!("Could not create FFmpeg archive file: {e}")))?;

    let mut buf = [0u8; 64 * 1024];
    let mut downloaded = 0u64;
    loop {
        let n = response
            .read(&mut buf)
            .map_err(|_| AppErrorDto::io_error("Failed while reading FFmpeg download stream"))?;
        if n == 0 {
            break;
        }
        file.write_all(&buf[..n])
            .map_err(|_| AppErrorDto::io_error("Failed while writing FFmpeg archive"))?;
        downloaded = downloaded.saturating_add(n as u64);
        on_progress("downloading", downloaded, total, "Downloading FFmpeg…");
    }
    drop(file);

    on_progress("unpacking", downloaded, total, "Unpacking FFmpeg…");
    extract_ffmpeg_exe(&archive_path, &dest_dir)?;
    let _ = fs::remove_file(&archive_path);

    if !is_ready() {
        return Err(AppErrorDto::io_error(
            "FFmpeg install finished but the binary is still missing",
        ));
    }
    on_progress("ready", downloaded, total, "FFmpeg is ready");
    Ok(())
}

#[cfg(windows)]
fn extract_ffmpeg_exe(archive: &Path, dest_dir: &Path) -> CmdResult<()> {
    let file = File::open(archive)
        .map_err(|_| AppErrorDto::io_error("Could not open downloaded FFmpeg archive"))?;
    let mut zip = zip::ZipArchive::new(file)
        .map_err(|_| AppErrorDto::io_error("Could not read FFmpeg zip archive"))?;

    let mut matched_index: Option<usize> = None;
    for i in 0..zip.len() {
        let name = zip
            .by_index(i)
            .map(|z| z.name().replace('\\', "/"))
            .unwrap_or_default();
        let lower = name.to_ascii_lowercase();
        if lower.ends_with("/bin/ffmpeg.exe") || lower.ends_with("ffmpeg.exe") {
            matched_index = Some(i);
            break;
        }
    }

    let Some(index) = matched_index else {
        return Err(AppErrorDto::io_error(
            "FFmpeg archive did not contain ffmpeg.exe",
        ));
    };

    let mut entry = zip
        .by_index(index)
        .map_err(|_| AppErrorDto::io_error("Could not read ffmpeg.exe from archive"))?;
    let out_path = dest_dir.join("ffmpeg.exe");
    let mut out = File::create(&out_path)
        .map_err(|e| AppErrorDto::io_error(format!("Could not write ffmpeg.exe: {e}")))?;
    copy(&mut entry, &mut out)
        .map_err(|_| AppErrorDto::io_error("Failed while extracting ffmpeg.exe"))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn status_reports_missing_or_ready() {
        let s = status();
        assert!(
            matches!(
                s.phase.as_str(),
                "missing" | "ready" | "error" | "starting" | "downloading" | "unpacking"
            ),
            "unexpected phase {}",
            s.phase
        );
        if s.installed {
            assert!(is_ready());
        }
    }

    #[test]
    fn init_install_dir_sets_managed_binary_path() {
        let dir =
            std::env::temp_dir().join(format!("meetphant-ffmpeg-init-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        init_install_dir(dir.clone());

        let path = managed_binary_path().expect("install dir set");
        assert_eq!(path.parent(), Some(dir.as_path()));
        #[cfg(windows)]
        assert_eq!(path.extension().and_then(|e| e.to_str()), Some("exe"));
        let _ = fs::remove_dir_all(&dir);
    }

    #[test]
    fn bundled_candidates_include_production_name() {
        let dir = PathBuf::from("C:\\fake\\app");
        let candidates = bundled_candidates_in(&dir);
        #[cfg(windows)]
        {
            assert!(candidates.iter().any(|p| p.ends_with("ffmpeg.exe")));
            assert!(candidates
                .iter()
                .any(|p| p.ends_with("ffmpeg-x86_64-pc-windows-msvc.exe")));
        }
        #[cfg(not(windows))]
        {
            assert!(candidates.iter().any(|p| p.ends_with("ffmpeg")));
        }
    }

    #[test]
    fn bundled_candidates_include_tauri_env_triple() {
        std::env::set_var("TAURI_ENV_TARGET_TRIPLE", "x86_64-pc-windows-msvc");
        let dir = PathBuf::from("/app");
        let candidates = bundled_candidates_in(&dir);
        std::env::remove_var("TAURI_ENV_TARGET_TRIPLE");
        assert!(
            candidates.iter().any(|p| {
                p.file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|n| n.starts_with("ffmpeg-x86_64-pc-windows-msvc"))
            }),
            "missing triple candidate: {candidates:?}"
        );
    }
}
