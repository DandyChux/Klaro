use crate::license::VersionLimits;
use crate::pii_detector::PiiDetector;
use crate::scrubber::Scrubber;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::mpsc::Sender;
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize)]
pub struct ProcessingProgress {
    pub bytes_processed: u64,
    pub total_bytes: u64,
    pub rows_processed: usize,
    pub pii_found: usize,
    pub current_phase: String,
    pub percent_complete: f32,
}

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct ProcessingStats {
    pub rows_processed: usize,
    pub rows_affected: usize,
    pub cells_affected: usize,
    pub total_pii_found: usize,
}

#[derive(Debug)]
pub enum ProcessError {
    Io(std::io::Error),
    Csv(csv::Error),
    LimitExceeded(String),
    Parse(String),
}

impl std::fmt::Display for ProcessError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ProcessError::Io(e) => write!(f, "IO error: {}", e),
            ProcessError::Csv(e) => write!(f, "CSV error: {}", e),
            ProcessError::LimitExceeded(msg) => write!(f, "{}", msg),
            ProcessError::Parse(msg) => write!(f, "Parse error: {}", msg),
        }
    }
}

impl From<std::io::Error> for ProcessError {
    fn from(e: std::io::Error) -> Self {
        ProcessError::Io(e)
    }
}

impl From<csv::Error> for ProcessError {
    fn from(e: csv::Error) -> Self {
        ProcessError::Csv(e)
    }
}

/// Simple temp file helper using std
pub struct TempFile {
    path: PathBuf,
}

impl TempFile {
    pub fn new(extension: &str) -> std::io::Result<Self> {
        let temp_dir = std::env::temp_dir();
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_nanos())
            .unwrap_or(0);
        let random: u64 = rand::rng().random();

        let filename = format!(
            "rusty_scrubber_{}_{}_{}.{}",
            std::process::id(),
            timestamp,
            random,
            extension
        );
        let path = temp_dir.join(filename);

        // Create empty file to reserve the path
        File::create(&path)?;

        Ok(Self { path })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempFile {
    fn drop(&mut self) {
        // Clean up temp file when done
        let _ = fs::remove_file(&self.path);
    }
}

pub struct StreamingProcessor {
    detector: PiiDetector,
    scrubber: Scrubber,
    limits: VersionLimits,
    chunk_size: usize,
}

impl StreamingProcessor {
    pub fn new(detector: PiiDetector, scrubber: Scrubber, limits: VersionLimits) -> Self {
        Self {
            detector,
            scrubber,
            limits,
            chunk_size: 64 * 1024, // 64KB chunks for better I/O performance
        }
    }

    /// Process a CSV file in streaming mode - never loads entire file into memory
    pub fn process_csv_streaming(
        &self,
        input_path: &Path,
        output_path: &Path,
        progress_tx: Option<Sender<ProcessingProgress>>,
    ) -> Result<ProcessingStats, ProcessError> {
        let file = File::open(input_path)?;
        let file_size = file.metadata()?.len();

        // Check file size limit
        self.limits
            .check_file_size(file_size)
            .map_err(|e| ProcessError::LimitExceeded(e.to_string()))?;

        let reader = BufReader::with_capacity(self.chunk_size, file);
        let output_file = File::create(output_path)?;
        let mut writer = BufWriter::with_capacity(self.chunk_size, output_file);

        let mut csv_reader = csv::ReaderBuilder::new()
            .flexible(true)
            .has_headers(true)
            .from_reader(reader);

        let mut csv_writer = csv::Writer::from_writer(&mut writer);

        let mut stats = ProcessingStats::default();

        // Process headers
        if let Ok(headers) = csv_reader.headers() {
            let scrubbed_headers: Vec<String> = headers
                .iter()
                .map(|h| {
                    let matches = self.detector.detect(h);
                    stats.total_pii_found += matches.len();
                    self.scrubber.scrub_text(h, &matches)
                })
                .collect();
            csv_writer.write_record(&scrubbed_headers)?;
        }

        // Process rows in streaming fashion
        for (row_idx, result) in csv_reader.records().enumerate() {
            // Check row limit
            if let Some(max_rows) = self.limits.max_rows_per_file {
                if row_idx >= max_rows {
                    return Err(ProcessError::LimitExceeded(format!(
                        "Row limit ({}) exceeded. Upgrade to Pro for unlimited rows.",
                        max_rows
                    )));
                }
            }

            let record = result?;
            let mut row_had_pii = false;

            let scrubbed_row: Vec<String> = record
                .iter()
                .map(|cell| {
                    let matches = self.detector.detect(cell);
                    if !matches.is_empty() {
                        row_had_pii = true;
                        stats.total_pii_found += matches.len();
                        stats.cells_affected += 1;
                    }
                    self.scrubber.scrub_text(cell, &matches)
                })
                .collect();

            if row_had_pii {
                stats.rows_affected += 1;
            }

            csv_writer.write_record(&scrubbed_row)?;
            stats.rows_processed += 1;

            // Update progress periodically (every 1000 rows)
            if row_idx % 1000 == 0 {
                if let Some(ref tx) = progress_tx {
                    let estimated_progress = (row_idx as f32 / (row_idx + 1000) as f32).min(0.99);
                    let _ = tx.send(ProcessingProgress {
                        bytes_processed: (file_size as f32 * estimated_progress) as u64,
                        total_bytes: file_size,
                        rows_processed: stats.rows_processed,
                        pii_found: stats.total_pii_found,
                        current_phase: "Processing rows...".to_string(),
                        percent_complete: estimated_progress * 100.0,
                    });
                }
            }
        }

        csv_writer.flush()?;
        writer.flush()?;

        // Send final progress
        if let Some(ref tx) = progress_tx {
            let _ = tx.send(ProcessingProgress {
                bytes_processed: file_size,
                total_bytes: file_size,
                rows_processed: stats.rows_processed,
                pii_found: stats.total_pii_found,
                current_phase: "Complete".to_string(),
                percent_complete: 100.0,
            });
        }

        Ok(stats)
    }

    /// Process a text file line by line
    pub fn process_text_streaming(
        &self,
        input_path: &Path,
        output_path: &Path,
        progress_tx: Option<Sender<ProcessingProgress>>,
    ) -> Result<ProcessingStats, ProcessError> {
        let file = File::open(input_path)?;
        let file_size = file.metadata()?.len();

        self.limits
            .check_file_size(file_size)
            .map_err(|e| ProcessError::LimitExceeded(e.to_string()))?;

        let reader = BufReader::with_capacity(self.chunk_size, file);
        let output_file = File::create(output_path)?;
        let mut writer = BufWriter::with_capacity(self.chunk_size, output_file);

        let mut stats = ProcessingStats::default();
        let mut bytes_processed: u64 = 0;

        for (line_idx, line_result) in reader.lines().enumerate() {
            let line = line_result?;
            bytes_processed += line.len() as u64 + 1; // +1 for newline

            let matches = self.detector.detect(&line);
            if !matches.is_empty() {
                stats.total_pii_found += matches.len();
                stats.rows_affected += 1;
            }

            let scrubbed = self.scrubber.scrub_text(&line, &matches);
            writeln!(writer, "{}", scrubbed)?;
            stats.rows_processed += 1;

            // Update progress periodically
            if line_idx % 10000 == 0 {
                if let Some(ref tx) = progress_tx {
                    let _ = tx.send(ProcessingProgress {
                        bytes_processed,
                        total_bytes: file_size,
                        rows_processed: stats.rows_processed,
                        pii_found: stats.total_pii_found,
                        current_phase: "Processing lines...".to_string(),
                        percent_complete: (bytes_processed as f32 / file_size as f32) * 100.0,
                    });
                }
            }
        }

        writer.flush()?;

        Ok(stats)
    }

    /// Process bytes directly (writes to temp, processes, reads back)
    /// Returns the scrubbed content as bytes
    pub fn process_bytes(
        &self,
        input_bytes: &[u8],
        file_extension: &str,
        progress_tx: Option<Sender<ProcessingProgress>>,
    ) -> Result<(Vec<u8>, ProcessingStats), ProcessError> {
        // Create temp files using std
        let input_temp = TempFile::new(file_extension)?;
        let output_temp = TempFile::new(file_extension)?;

        // Write input bytes
        fs::write(input_temp.path(), input_bytes)?;

        // Process based on file type
        let stats = match file_extension.to_lowercase().as_str() {
            "csv" => {
                self.process_csv_streaming(input_temp.path(), output_temp.path(), progress_tx)?
            }
            "txt" | "json" => {
                self.process_text_streaming(input_temp.path(), output_temp.path(), progress_tx)?
            }
            _ => self.process_text_streaming(input_temp.path(), output_temp.path(), progress_tx)?,
        };

        // Read output
        let output_bytes = fs::read(output_temp.path())?;

        // Temp files are automatically cleaned up when TempFile is dropped
        Ok((output_bytes, stats))
    }
}
