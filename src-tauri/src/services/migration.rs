//! One-time migration of on-disk state from the pre-rename `Meetly` app
//! identity to `Meetphant`. Every step is best-effort and idempotent: if the
//! legacy data is missing, or the new location is already populated, the
//! step is skipped silently so a fresh install behaves identically.

use std::path::Path;

const LEGACY_IDENTIFIER: &str = "com.meetly.app";
const LEGACY_DB_FILE: &str = "meetly.db";
const LEGACY_RECORDINGS_DIR_NAME: &str = "Meetly";
const CURRENT_RECORDINGS_DIR_NAME: &str = "Meetphant";

/// Copy the legacy SQLite database and any already-downloaded FFmpeg binary
/// into the new `com.meetphant.app` data directory. No-op once the new
/// database file already exists.
pub fn migrate_app_data_dir(new_data_dir: &Path, new_db_path: &Path) {
    if new_db_path.exists() {
        return;
    }
    let Some(base) = new_data_dir.parent() else {
        return;
    };
    let legacy_dir = base.join(LEGACY_IDENTIFIER);
    let legacy_db = legacy_dir.join(LEGACY_DB_FILE);
    if !legacy_db.exists() || std::fs::copy(&legacy_db, new_db_path).is_err() {
        return;
    }

    let legacy_ffmpeg = legacy_dir.join("ffmpeg");
    let new_ffmpeg = new_data_dir.join("ffmpeg");
    if legacy_ffmpeg.is_dir() && !new_ffmpeg.exists() {
        let _ = copy_dir_all(&legacy_ffmpeg, &new_ffmpeg);
    }
}

fn copy_dir_all(src: &Path, dst: &Path) -> std::io::Result<()> {
    std::fs::create_dir_all(dst)?;
    for entry in std::fs::read_dir(src)? {
        let entry = entry?;
        let path = entry.path();
        let dest_path = dst.join(entry.file_name());
        if path.is_dir() {
            copy_dir_all(&path, &dest_path)?;
        } else {
            std::fs::copy(&path, &dest_path)?;
        }
    }
    Ok(())
}

/// Rename the legacy `Documents/Meetly` folder to `Documents/Meetphant` so
/// previously recorded audio (default location only) moves with the app.
/// Skipped if the new folder already exists.
pub fn migrate_recordings_dir() {
    let Some(docs) = dirs::document_dir() else {
        return;
    };
    let legacy = docs.join(LEGACY_RECORDINGS_DIR_NAME);
    let current = docs.join(CURRENT_RECORDINGS_DIR_NAME);
    if legacy.exists() && !current.exists() {
        let _ = std::fs::rename(&legacy, &current);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_dir(name: &str) -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!("{name}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn migrates_db_and_ffmpeg_dir_from_legacy_location() {
        let base = temp_dir("meetphant-migration-base");
        let legacy_dir = base.join(LEGACY_IDENTIFIER);
        fs::create_dir_all(legacy_dir.join("ffmpeg")).unwrap();
        fs::write(legacy_dir.join(LEGACY_DB_FILE), b"legacy-db").unwrap();
        fs::write(legacy_dir.join("ffmpeg").join("ffmpeg.exe"), b"bin").unwrap();

        let new_dir = base.join("com.meetphant.app");
        fs::create_dir_all(&new_dir).unwrap();
        let new_db = new_dir.join("meetphant.db");

        migrate_app_data_dir(&new_dir, &new_db);

        assert!(new_db.exists());
        assert_eq!(fs::read(&new_db).unwrap(), b"legacy-db");
        assert!(new_dir.join("ffmpeg").join("ffmpeg.exe").exists());

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn skips_when_new_db_already_exists() {
        let base = temp_dir("meetphant-migration-skip");
        let legacy_dir = base.join(LEGACY_IDENTIFIER);
        fs::create_dir_all(&legacy_dir).unwrap();
        fs::write(legacy_dir.join(LEGACY_DB_FILE), b"legacy-db").unwrap();

        let new_dir = base.join("com.meetphant.app");
        fs::create_dir_all(&new_dir).unwrap();
        let new_db = new_dir.join("meetphant.db");
        fs::write(&new_db, b"already-here").unwrap();

        migrate_app_data_dir(&new_dir, &new_db);

        assert_eq!(fs::read(&new_db).unwrap(), b"already-here");

        let _ = fs::remove_dir_all(&base);
    }

    #[test]
    fn skips_when_no_legacy_db_present() {
        let base = temp_dir("meetphant-migration-none");
        let new_dir = base.join("com.meetphant.app");
        fs::create_dir_all(&new_dir).unwrap();
        let new_db = new_dir.join("meetphant.db");

        migrate_app_data_dir(&new_dir, &new_db);

        assert!(!new_db.exists());

        let _ = fs::remove_dir_all(&base);
    }
}
