pub mod debouncer;

use crate::converter::{ConvertOptions, Format};
use crate::error::MartiniError;
use debouncer::DebounceQueue;
use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use std::fs;
use std::path::{Path, PathBuf};
use std::str::FromStr;
use std::sync::mpsc::{Receiver, channel};
use std::time::{Duration, Instant};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OriginalAction {
    Keep,
    Delete,
    MoveTo(PathBuf),
}

#[derive(Debug, Clone)]
pub struct WatchRule {
    pub input_dir: PathBuf,
    pub output_dir: PathBuf,
    pub recursive: bool,
    pub settle_duration: Duration,
    pub original_action: OriginalAction,
    pub overwrite: bool,
    pub format_override: Option<Format>,
    pub initial_scan: bool,
}

pub struct DirectoryWatcher {
    pub rule: WatchRule,
}

impl DirectoryWatcher {
    pub fn new(rule: WatchRule) -> Self {
        Self { rule }
    }

    pub fn run(&self, shutdown_signal: Receiver<()>) -> Result<(), MartiniError> {
        self.run_internal(shutdown_signal, None)
    }

    pub fn run_with_timeout(
        &self,
        shutdown_signal: Receiver<()>,
        timeout: Duration,
    ) -> Result<(), MartiniError> {
        self.run_internal(shutdown_signal, Some(timeout))
    }

    fn run_internal(
        &self,
        shutdown_signal: Receiver<()>,
        timeout: Option<Duration>,
    ) -> Result<(), MartiniError> {
        if !self.rule.input_dir.exists() {
            return Err(MartiniError::InputFileNotFound {
                path: self.rule.input_dir.to_string_lossy().to_string(),
            });
        }

        if !self.rule.output_dir.exists() {
            fs::create_dir_all(&self.rule.output_dir).map_err(MartiniError::Io)?;
        }

        let canonical_input = self
            .rule
            .input_dir
            .canonicalize()
            .unwrap_or_else(|_| self.rule.input_dir.clone());
        let canonical_output = self
            .rule
            .output_dir
            .canonicalize()
            .unwrap_or_else(|_| self.rule.output_dir.clone());

        let mut queue = DebounceQueue::new(self.rule.settle_duration);

        // Initial scan for existing files if requested
        if self.rule.initial_scan {
            let mut existing = Vec::new();
            scan_dir(&canonical_input, self.rule.recursive, &mut existing);
            for path in existing {
                if !is_excluded(&path, &canonical_output, &self.rule.original_action) {
                    self.process_file(&path, &canonical_input, &canonical_output)?;
                }
            }
        }

        let (notify_tx, notify_rx) = channel();
        let mut watcher =
            RecommendedWatcher::new(notify_tx, notify::Config::default()).map_err(|e| {
                MartiniError::InvalidInputData {
                    reason: format!("Failed to create watcher: {}", e),
                }
            })?;

        let mode = if self.rule.recursive {
            RecursiveMode::Recursive
        } else {
            RecursiveMode::NonRecursive
        };

        watcher
            .watch(&canonical_input, mode)
            .map_err(|e| MartiniError::InvalidInputData {
                reason: format!("Failed to watch input directory: {}", e),
            })?;

        let start_time = Instant::now();

        loop {
            // Check shutdown signal
            if shutdown_signal.try_recv().is_ok() {
                break;
            }

            // Check overall timeout
            if timeout.is_some_and(|t| start_time.elapsed() >= t && queue.is_empty()) {
                break;
            }

            // Poll file events with timeout
            if let Ok(Ok(event)) = notify_rx.recv_timeout(Duration::from_millis(50))
                && matches!(
                    event.kind,
                    EventKind::Create(_) | EventKind::Modify(_) | EventKind::Any
                )
            {
                for path in event.paths {
                    if path.is_file()
                        && !is_excluded(&path, &canonical_output, &self.rule.original_action)
                    {
                        queue.insert_or_update(path);
                    }
                }
            }

            // Process settled files
            let settled = queue.pop_settled();
            for path in settled {
                if path.is_file() {
                    let _ = self.process_file(&path, &canonical_input, &canonical_output);
                }
            }
        }

        Ok(())
    }

    fn process_file(
        &self,
        input_path: &Path,
        canonical_input: &Path,
        canonical_output: &Path,
    ) -> Result<(), MartiniError> {
        let ext = input_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("")
            .to_lowercase();
        let from_format = match Format::from_str(&ext) {
            Ok(f) => f,
            Err(_) => return Ok(()), // Skip unsupported file types
        };

        let to_format = match self.rule.format_override {
            Some(f) => f,
            None => return Ok(()),
        };
        // Determine destination file path
        let rel_path = input_path
            .strip_prefix(canonical_input)
            .unwrap_or_else(|_| Path::new(input_path.file_name().unwrap()));
        let mut out_file = canonical_output.join(rel_path);
        out_file.set_extension(to_format.to_string());

        if out_file.exists() && !self.rule.overwrite {
            return Ok(());
        }

        let opt = ConvertOptions {
            input_path: input_path.to_path_buf(),
            output_path: out_file.clone(),
            overwrite: self.rule.overwrite,
            ..Default::default()
        };

        let input_bytes = fs::read(input_path).map_err(MartiniError::Io)?;
        crate::converter::convert(from_format, to_format, &input_bytes, &opt)?;

        // Perform original action
        match &self.rule.original_action {
            OriginalAction::Keep => {}
            OriginalAction::Delete => {
                let _ = fs::remove_file(input_path);
            }
            OriginalAction::MoveTo(dest_dir) => {
                let dest_path = dest_dir.join(rel_path);
                if let Some(p) = dest_path.parent() {
                    let _ = fs::create_dir_all(p);
                }
                let _ = fs::rename(input_path, dest_path);
            }
        }

        Ok(())
    }
}

fn is_excluded(path: &Path, canonical_output: &Path, action: &OriginalAction) -> bool {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    if canonical.starts_with(canonical_output) {
        return true;
    }
    if let OriginalAction::MoveTo(move_dir) = action {
        let canonical_move = move_dir.canonicalize().unwrap_or_else(|_| move_dir.clone());
        if canonical.starts_with(canonical_move) {
            return true;
        }
    }
    false
}

fn scan_dir(dir: &Path, recursive: bool, files: &mut Vec<PathBuf>) {
    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() && recursive {
                scan_dir(&path, recursive, files);
            } else if path.is_file() {
                files.push(path);
            }
        }
    }
}
