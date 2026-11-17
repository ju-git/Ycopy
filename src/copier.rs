//! Copy engine: GUI-agnostic and language-agnostic.
//!
//! Each source file is read **once**, in fixed-size chunks, and every chunk is
//! written to both destinations. Memory use is bounded by [`BUFFER_SIZE`],
//! whatever the file size.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Size of the single read buffer shared by both destinations.
pub const BUFFER_SIZE: usize = 4 * 1024 * 1024;

/// Minimum delay between two [`Event::Progress`] notifications.
const PROGRESS_INTERVAL: Duration = Duration::from_millis(100);

#[derive(Debug, Clone)]
pub struct Job {
    pub files: Vec<PathBuf>,
    pub dest1: PathBuf,
    pub dest2: PathBuf,
}

#[derive(Debug)]
pub enum CopyError {
    /// An I/O error, with the path it relates to.
    Io { path: PathBuf, source: io::Error },
    /// The source file is the same as one of the destination files.
    SameFile(PathBuf),
    /// The source path has no file name (e.g. `C:\`).
    NoFileName(PathBuf),
    /// The operation was cancelled by the caller.
    Cancelled,
}

impl CopyError {
    fn io(path: &Path, source: io::Error) -> Self {
        CopyError::Io { path: path.to_path_buf(), source }
    }
}

impl std::fmt::Display for CopyError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CopyError::Io { path, source } => write!(f, "{}: {source}", path.display()),
            CopyError::SameFile(p) => write!(f, "{}: source and destination are the same file", p.display()),
            CopyError::NoFileName(p) => write!(f, "{}: no file name", p.display()),
            CopyError::Cancelled => write!(f, "cancelled"),
        }
    }
}

impl std::error::Error for CopyError {}

/// Snapshot of the global progress of a job.
#[derive(Debug, Clone, Copy, Default)]
pub struct Progress {
    /// Index (in `Job::files`) of the file being copied.
    pub file_index: usize,
    pub total_files: usize,
    /// Source bytes read so far (each byte is written to both destinations).
    pub bytes_done: u64,
    /// Bytes of files that failed, counted as consumed so the ETA stays meaningful.
    pub bytes_skipped: u64,
    pub bytes_total: u64,
    pub elapsed: Duration,
}

impl Progress {
    /// Source throughput in bytes per second (average since the job started).
    pub fn rate(&self) -> f64 {
        let secs = self.elapsed.as_secs_f64();
        if secs > 0.0 { self.bytes_done as f64 / secs } else { 0.0 }
    }

    /// Estimated remaining time, if a throughput can be computed.
    pub fn eta(&self) -> Option<Duration> {
        let rate = self.rate();
        if rate <= 0.0 {
            return None;
        }
        let left = self.bytes_total.saturating_sub(self.bytes_done + self.bytes_skipped);
        Some(Duration::from_secs_f64(left as f64 / rate))
    }

    /// Completed fraction in `0.0..=1.0`.
    pub fn fraction(&self) -> f64 {
        if self.bytes_total == 0 {
            return 1.0;
        }
        ((self.bytes_done + self.bytes_skipped) as f64 / self.bytes_total as f64).min(1.0)
    }
}

pub enum Event<'a> {
    Progress(Progress),
    FileDone { index: usize },
    FileFailed { index: usize, error: &'a CopyError },
}

#[derive(Debug, Default)]
pub struct Summary {
    pub copied: usize,
    pub failed: Vec<(usize, CopyError)>,
    pub cancelled: bool,
    pub bytes: u64,
    pub elapsed: Duration,
}

impl Summary {
    pub fn rate(&self) -> f64 {
        let secs = self.elapsed.as_secs_f64();
        if secs > 0.0 { self.bytes as f64 / secs } else { 0.0 }
    }
}

/// Runs a job: files are copied one after the other. A failing file is
/// reported and skipped; cancelling stops at the next chunk.
///
/// Returns `Err` only if a destination folder cannot be created.
pub fn run(
    job: &Job,
    cancel: &AtomicBool,
    observer: &mut dyn FnMut(Event<'_>),
) -> Result<Summary, CopyError> {
    for dir in [&job.dest1, &job.dest2] {
        fs::create_dir_all(dir).map_err(|e| CopyError::io(dir, e))?;
    }

    let start = Instant::now();
    let bytes_total: u64 = job
        .files
        .iter()
        .filter_map(|p| fs::metadata(p).ok())
        .map(|m| m.len())
        .sum();

    let mut progress = Progress {
        total_files: job.files.len(),
        bytes_total,
        ..Progress::default()
    };
    let mut summary = Summary::default();
    let mut buffer = vec![0u8; BUFFER_SIZE];
    let mut last_emit = Instant::now() - PROGRESS_INTERVAL;

    for (index, src) in job.files.iter().enumerate() {
        if cancel.load(Ordering::Relaxed) {
            summary.cancelled = true;
            break;
        }
        progress.file_index = index;
        let done_before = progress.bytes_done;

        let result = copy_one(src, &job.dest1, &job.dest2, &mut buffer, cancel, &mut |n| {
            progress.bytes_done += n;
            progress.elapsed = start.elapsed();
            if last_emit.elapsed() >= PROGRESS_INTERVAL {
                last_emit = Instant::now();
                observer(Event::Progress(progress));
            }
        });

        progress.elapsed = start.elapsed();
        match result {
            Ok(()) => {
                summary.copied += 1;
                observer(Event::FileDone { index });
                observer(Event::Progress(progress));
            }
            Err(CopyError::Cancelled) => {
                summary.cancelled = true;
                break;
            }
            Err(error) => {
                if let Ok(meta) = fs::metadata(src) {
                    let copied_of_file = progress.bytes_done - done_before;
                    progress.bytes_skipped += meta.len().saturating_sub(copied_of_file);
                }
                observer(Event::FileFailed { index, error: &error });
                observer(Event::Progress(progress));
                summary.failed.push((index, error));
            }
        }
    }

    summary.bytes = progress.bytes_done;
    summary.elapsed = start.elapsed();
    Ok(summary)
}

/// Copies one file to `dir1` and `dir2` with a single read pass.
fn copy_one(
    src: &Path,
    dir1: &Path,
    dir2: &Path,
    buffer: &mut [u8],
    cancel: &AtomicBool,
    on_bytes: &mut dyn FnMut(u64),
) -> Result<(), CopyError> {
    let name = src.file_name().ok_or_else(|| CopyError::NoFileName(src.to_path_buf()))?;
    let (dst1, dst2) = (dir1.join(name), dir2.join(name));

    for dst in [&dst1, &dst2] {
        if is_same_file(src, dst) {
            return Err(CopyError::SameFile(src.to_path_buf()));
        }
    }

    let mut input = File::open(src).map_err(|e| CopyError::io(src, e))?;
    let modified = input.metadata().and_then(|m| m.modified()).ok();

    let result = (|| {
        let mut out1 = File::create(&dst1).map_err(|e| CopyError::io(&dst1, e))?;
        let mut out2 = File::create(&dst2).map_err(|e| CopyError::io(&dst2, e))?;

        loop {
            let n = input.read(buffer).map_err(|e| CopyError::io(src, e))?;
            if n == 0 {
                break;
            }
            out1.write_all(&buffer[..n]).map_err(|e| CopyError::io(&dst1, e))?;
            out2.write_all(&buffer[..n]).map_err(|e| CopyError::io(&dst2, e))?;
            on_bytes(n as u64);
            if cancel.load(Ordering::Relaxed) {
                return Err(CopyError::Cancelled);
            }
        }

        if let Some(t) = modified {
            // Preserving the modification time is best effort.
            let _ = out1.set_modified(t);
            let _ = out2.set_modified(t);
        }
        out1.flush().map_err(|e| CopyError::io(&dst1, e))?;
        out2.flush().map_err(|e| CopyError::io(&dst2, e))?;
        Ok(())
    })();

    if result.is_err() {
        // Do not leave truncated copies behind.
        let _ = fs::remove_file(&dst1);
        let _ = fs::remove_file(&dst2);
    }
    result
}

fn is_same_file(src: &Path, dst: &Path) -> bool {
    match (fs::canonicalize(src), fs::canonicalize(dst)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temp_dir(tag: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("ycopy-test-{tag}-{}", std::process::id()));
        let _ = fs::remove_dir_all(&dir);
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn copies_to_both_destinations() {
        let root = temp_dir("both");
        let src = root.join("a.bin");
        // Larger than one buffer to exercise the chunk loop.
        let data: Vec<u8> = (0..BUFFER_SIZE * 2 + 123).map(|i| (i % 251) as u8).collect();
        fs::write(&src, &data).unwrap();

        let job = Job { files: vec![src], dest1: root.join("d1"), dest2: root.join("d2") };
        let cancel = AtomicBool::new(false);
        let mut done = 0;
        let summary = run(&job, &cancel, &mut |e| {
            if matches!(e, Event::FileDone { .. }) {
                done += 1;
            }
        })
        .unwrap();

        assert_eq!(summary.copied, 1);
        assert_eq!(done, 1);
        assert!(summary.failed.is_empty());
        assert_eq!(fs::read(root.join("d1/a.bin")).unwrap(), data);
        assert_eq!(fs::read(root.join("d2/a.bin")).unwrap(), data);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn refuses_to_copy_onto_itself() {
        let root = temp_dir("same");
        let src = root.join("a.txt");
        fs::write(&src, b"hello").unwrap();

        let job = Job { files: vec![src.clone()], dest1: root.clone(), dest2: root.join("d2") };
        let cancel = AtomicBool::new(false);
        let summary = run(&job, &cancel, &mut |_| {}).unwrap();

        assert_eq!(summary.failed.len(), 1);
        assert!(matches!(summary.failed[0].1, CopyError::SameFile(_)));
        assert_eq!(fs::read(&src).unwrap(), b"hello", "source must be intact");
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn missing_source_is_reported_and_next_file_still_copied() {
        let root = temp_dir("missing");
        let ok = root.join("ok.txt");
        fs::write(&ok, b"data").unwrap();

        let job = Job {
            files: vec![root.join("nope.txt"), ok],
            dest1: root.join("d1"),
            dest2: root.join("d2"),
        };
        let cancel = AtomicBool::new(false);
        let summary = run(&job, &cancel, &mut |_| {}).unwrap();

        assert_eq!(summary.copied, 1);
        assert_eq!(summary.failed.len(), 1);
        assert_eq!(summary.failed[0].0, 0);
        let _ = fs::remove_dir_all(root);
    }

    #[test]
    fn cancel_stops_before_any_copy() {
        let root = temp_dir("cancel");
        let src = root.join("a.txt");
        fs::write(&src, b"data").unwrap();

        let job = Job { files: vec![src], dest1: root.join("d1"), dest2: root.join("d2") };
        let cancel = AtomicBool::new(true);
        let summary = run(&job, &cancel, &mut |_| {}).unwrap();

        assert!(summary.cancelled);
        assert_eq!(summary.copied, 0);
        let _ = fs::remove_dir_all(root);
    }
}
