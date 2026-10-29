//! First-run acquisition of the baseline SoundFont.
//!
//! The repository ships no large binary. On first launch the app downloads the MIT-licensed
//! FluidR3 General MIDI SoundFont into its app-data directory, verifying the bytes against a
//! pinned SHA-256 so a corrupted or substituted download is discarded.
//!
//! Attribution: Fluid (R3) SoundFont, Copyright (c) 2000-2002, 2008 Frank Wen. See `NOTICE`.

use std::fs::{self, File};
use std::io::{BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use sha2::{Digest, Sha256};

/// The pinned release asset: the FluidR3 GM SoundFont as published by the Ubuntu package.
const SOUNDFONT_URL: &str =
    "https://github.com/pianobooster/fluid-soundfont/releases/download/v3.1/FluidR3_GM.sf2";

/// SHA-256 of the pinned asset. A download that does not match is rejected.
const SOUNDFONT_SHA256: &str = "74594e8f4250680adf590507a306655a299935343583256f3b722c48a1bc1cb0";

/// File name the SoundFont is stored under in the app-data directory.
const SOUNDFONT_FILE: &str = "FluidR3_GM.sf2";

/// Bytes read per pass while downloading.
const CHUNK: usize = 64 * 1024;

/// Whether the SoundFont is installed and where it would live, as the frontend sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SoundfontStatus {
    pub installed: bool,
    pub path: String,
}

/// Errors from downloading the SoundFont.
#[derive(Debug)]
pub enum SoundfontError {
    /// The request failed or the server returned an error status.
    Http(Box<ureq::Error>),
    /// The downloaded bytes could not be saved.
    Io(std::io::Error),
    /// The downloaded bytes did not match the pinned checksum.
    Checksum { expected: String, actual: String },
    /// The download was cancelled.
    Cancelled,
}

impl std::fmt::Display for SoundfontError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            SoundfontError::Http(err) => write!(f, "could not download the SoundFont: {err}"),
            SoundfontError::Io(err) => write!(f, "could not save the SoundFont: {err}"),
            SoundfontError::Checksum { expected, actual } => write!(
                f,
                "the downloaded SoundFont failed its checksum: expected {expected}, got {actual}"
            ),
            SoundfontError::Cancelled => write!(f, "the SoundFont download was cancelled"),
        }
    }
}

impl std::error::Error for SoundfontError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            SoundfontError::Http(err) => Some(err.as_ref()),
            SoundfontError::Io(err) => Some(err),
            SoundfontError::Checksum { .. } | SoundfontError::Cancelled => None,
        }
    }
}

impl From<std::io::Error> for SoundfontError {
    fn from(err: std::io::Error) -> Self {
        SoundfontError::Io(err)
    }
}

impl From<ureq::Error> for SoundfontError {
    fn from(err: ureq::Error) -> Self {
        SoundfontError::Http(Box::new(err))
    }
}

/// Where the SoundFont lives for a given app-data directory.
pub fn installed_path(app_data: &Path) -> PathBuf {
    app_data.join(SOUNDFONT_FILE)
}

/// Whether the SoundFont is already present.
pub fn is_installed(app_data: &Path) -> bool {
    installed_path(app_data).is_file()
}

/// Reports whether the SoundFont is installed and where it would live.
pub fn status(app_data: &Path) -> SoundfontStatus {
    SoundfontStatus {
        installed: is_installed(app_data),
        path: installed_path(app_data).to_string_lossy().into_owned(),
    }
}

/// Downloads the SoundFont into `app_data`, verifies its checksum, and returns its path.
///
/// `cancelled` is polled while bytes arrive, so a caller can abort; `on_progress` receives the
/// running byte count and the total (zero when the server sends no length).
pub fn download(
    app_data: &Path,
    cancelled: &AtomicBool,
    on_progress: &mut dyn FnMut(u64, u64),
) -> Result<PathBuf, SoundfontError> {
    fs::create_dir_all(app_data)?;
    let destination = installed_path(app_data);
    // A temporary file means an interrupted download never looks installed.
    let temporary = destination.with_extension("part");

    let response = ureq::get(SOUNDFONT_URL).call()?;
    let total = response
        .header("Content-Length")
        .and_then(|value| value.parse().ok())
        .unwrap_or(0);
    let mut reader = response.into_reader();

    let hash = match write_and_hash(&mut reader, &temporary, total, cancelled, on_progress) {
        Ok(hash) => hash,
        Err(err) => {
            let _ = fs::remove_file(&temporary);
            return Err(err);
        }
    };

    if let Err(err) = verify(&hash) {
        let _ = fs::remove_file(&temporary);
        return Err(err);
    }

    fs::rename(&temporary, &destination)?;
    Ok(destination)
}

/// Streams the download into `temporary`, hashing as it goes, and returns the digest.
fn write_and_hash(
    reader: &mut impl Read,
    temporary: &Path,
    total: u64,
    cancelled: &AtomicBool,
    on_progress: &mut dyn FnMut(u64, u64),
) -> Result<[u8; 32], SoundfontError> {
    let mut file = BufWriter::new(File::create(temporary)?);
    let hash = copy_hashed(reader, &mut file, total, cancelled, on_progress)?;
    file.flush()?;
    Ok(hash)
}

/// Copies `reader` into `writer`, hashing the bytes, reporting progress, and returns the digest.
fn copy_hashed(
    reader: &mut impl Read,
    writer: &mut impl Write,
    total: u64,
    cancelled: &AtomicBool,
    on_progress: &mut dyn FnMut(u64, u64),
) -> Result<[u8; 32], SoundfontError> {
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; CHUNK];
    let mut received = 0u64;
    loop {
        if cancelled.load(Ordering::Relaxed) {
            return Err(SoundfontError::Cancelled);
        }
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        writer.write_all(&buffer[..read])?;
        hasher.update(&buffer[..read]);
        received += read as u64;
        on_progress(received, total);
    }
    Ok(hasher.finalize().into())
}

/// Checks a computed digest against the pinned value.
fn verify(hash: &[u8; 32]) -> Result<(), SoundfontError> {
    let actual = to_hex(hash);
    if actual == SOUNDFONT_SHA256 {
        Ok(())
    } else {
        Err(SoundfontError::Checksum {
            expected: SOUNDFONT_SHA256.to_string(),
            actual,
        })
    }
}

/// Lowercase hex of a byte string.
fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

#[cfg(test)]
mod tests {
    use super::{
        SOUNDFONT_FILE, SoundfontError, copy_hashed, installed_path, is_installed, to_hex,
    };
    use sha2::{Digest, Sha256};
    use std::io::Cursor;
    use std::sync::atomic::AtomicBool;

    #[test]
    fn builds_and_detects_the_install_path() {
        let directory = std::path::Path::new("/data");
        assert_eq!(installed_path(directory), directory.join(SOUNDFONT_FILE));
        assert!(!is_installed(directory));
    }

    #[test]
    fn hashes_and_reports_progress_while_streaming() {
        let data = vec![7_u8; 100_000];
        let mut reader = Cursor::new(data.clone());
        let mut written = Vec::new();
        let mut reports = Vec::new();

        let hash = copy_hashed(
            &mut reader,
            &mut written,
            100_000,
            &AtomicBool::new(false),
            &mut |received, total| reports.push((received, total)),
        )
        .expect("streaming into a Vec cannot fail");

        assert_eq!(written, data);
        assert_eq!(to_hex(&hash), to_hex(&Sha256::digest(&data)));
        // 100_000 bytes at 64 KiB per read: one full chunk, then the remainder.
        assert_eq!(reports, vec![(65_536, 100_000), (100_000, 100_000)]);
    }

    #[test]
    fn a_cancelled_download_stops_before_writing() {
        let mut reader = Cursor::new(vec![1_u8; 10]);
        let mut written = Vec::new();

        let result = copy_hashed(
            &mut reader,
            &mut written,
            10,
            &AtomicBool::new(true),
            &mut |_, _| {},
        );

        assert!(matches!(result, Err(SoundfontError::Cancelled)));
        assert!(written.is_empty());
    }

    #[test]
    fn rejects_a_digest_that_does_not_match_the_pin() {
        let error = super::verify(&[0_u8; 32]).expect_err("a zero digest must not match");
        match error {
            SoundfontError::Checksum { actual, .. } => assert_eq!(actual, "00".repeat(32)),
            other => panic!("expected a checksum error, got {other:?}"),
        }
    }

    #[test]
    fn formats_bytes_as_lowercase_hex() {
        assert_eq!(to_hex(&[0x00, 0x0f, 0xff]), "000fff");
    }
}
