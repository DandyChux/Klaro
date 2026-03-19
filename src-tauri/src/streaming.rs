use crate::license::VersionLimits;
use crate::ner::{NerEntityType, is_ner_available, predict_entities};
use crate::pii_detector::{NER_BATCH_SIZE, PiiDetector, PiiMatch, PiiType};
use crate::scrubber::Scrubber;
use rand::Rng;
use serde::{Deserialize, Serialize};
use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Read, Write};
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
        let separator = " | ";
        let ner_enabled = self.detector.uses_ner() && is_ner_available();

        let file = File::open(input_path)?;
        let file_size = file.metadata()?.len();

        // Check file size limit
        self.limits
            .check_file_size(file_size)
            .map_err(|e| ProcessError::LimitExceeded(e.to_string()))?;

        let reader = BufReader::with_capacity(self.chunk_size, file);
        let output_file = File::create(output_path)?;
        let writer = BufWriter::with_capacity(self.chunk_size, output_file);

        let mut csv_reader = csv::ReaderBuilder::new()
            .flexible(true)
            .has_headers(true)
            .from_reader(reader);

        let mut csv_writer = csv::Writer::from_writer(writer);
        let mut stats = ProcessingStats::default();

        // Process headers
        if let Ok(headers) = csv_reader.headers() {
            let scrubbed_headers: Vec<String> = headers
                .iter()
                .map(|h| {
                    let matches = self.detector.detect(&[h]);
                    stats.total_pii_found += matches.len();
                    self.scrubber.scrub_text(h, &matches)
                })
                .collect();
            csv_writer.write_record(&scrubbed_headers)?;
        }

        // Read rows in micro-batches
        let mut row_buffer: Vec<csv::StringRecord> = Vec::with_capacity(NER_BATCH_SIZE);
        let mut global_row_idx: usize = 0;

        // Process rows in streaming fashion
        for result in csv_reader.records() {
            // Check row limit
            if let Some(max_rows) = self.limits.max_rows_per_file {
                if global_row_idx >= max_rows {
                    return Err(ProcessError::LimitExceeded(format!(
                        "Row limit ({}) exceeded. Upgrade to Pro for unlimited rows.",
                        max_rows
                    )));
                }
            }

            row_buffer.push(result?);
            global_row_idx += 1;

            if row_buffer.len() >= NER_BATCH_SIZE {
                self.scrub_csv(
                    &row_buffer,
                    &mut csv_writer,
                    &mut stats,
                    ner_enabled,
                    separator,
                )?;

                if let Some(ref tx) = progress_tx {
                    let est = (stats.rows_processed as f32 / (stats.rows_processed + 1000) as f32)
                        .min(0.99);

                    let _ = tx.send(ProcessingProgress {
                        bytes_processed: (file_size as f32 * est) as u64,
                        total_bytes: file_size,
                        rows_processed: stats.rows_processed,
                        pii_found: stats.total_pii_found,
                        current_phase: "Processing rows...".to_string(),
                        percent_complete: est * 100.0,
                    });
                }

                row_buffer.clear();
            }
        }

        // Flush remaining rows
        if !row_buffer.is_empty() {
            self.scrub_csv(
                &row_buffer,
                &mut csv_writer,
                &mut stats,
                ner_enabled,
                separator,
            )?;
        }

        csv_writer.flush()?;

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

    /// Process a micro-batch of CSV rows: regex per cell, one batched NER
    /// call per batch (joining cells per row), merge + scrub + write.
    fn scrub_csv<W: Write>(
        &self,
        rows: &[csv::StringRecord],
        writer: &mut csv::Writer<W>,
        stats: &mut ProcessingStats,
        ner_enabled: bool,
        separator: &str,
    ) -> Result<(), ProcessError> {
        // Step 1: regex per cell + build joined strings for NER
        let mut all_regex: Vec<Vec<Vec<PiiMatch>>> = Vec::with_capacity(rows.len());
        let mut joined_strings: Vec<String> = Vec::with_capacity(rows.len());
        let mut all_offsets: Vec<Vec<(usize, usize)>> = Vec::with_capacity(rows.len());

        for record in rows {
            let cell_regex: Vec<Vec<PiiMatch>> = record
                .iter()
                .map(|cell| self.detector.detect_regex(cell))
                .collect();
            all_regex.push(cell_regex);

            if ner_enabled {
                let mut joined = String::new();
                let mut offsets = Vec::new();
                for (idx, cell) in record.iter().enumerate() {
                    if idx > 0 {
                        joined.push_str(separator);
                    }
                    let start = joined.len();
                    joined.push_str(cell);
                    offsets.push((start, joined.len()));
                }
                joined_strings.push(joined);
                all_offsets.push(offsets);
            }
        }

        // Step 2: batched NER (direct, no cache — rows are unique in streaming)
        let ner_results: Vec<Vec<PiiMatch>> = if ner_enabled {
            let refs: Vec<&str> = joined_strings.iter().map(|s| s.as_str()).collect();
            match predict_entities(&refs) {
                Ok(batch_entities) => batch_entities
                    .into_iter()
                    .map(|entities| {
                        entities
                            .into_iter()
                            .filter_map(|entity| {
                                let pii_type = match entity.entity_type {
                                    NerEntityType::Person => Some(PiiType::Name),
                                    NerEntityType::Location => Some(PiiType::Address),
                                    NerEntityType::Organization => Some(PiiType::Organization),
                                    _ => None,
                                };
                                pii_type.and_then(|pt| {
                                    (entity.confidence >= 0.70).then_some(PiiMatch {
                                        pii_type: pt,
                                        value: entity.text,
                                        start: entity.start,
                                        end: entity.end,
                                    })
                                })
                            })
                            .collect()
                    })
                    .collect(),
                Err(_) => vec![Vec::new(); rows.len()],
            }
        } else {
            vec![Vec::new(); rows.len()]
        };

        // Step 3: merge + scrub + write
        for (i, record) in rows.iter().enumerate() {
            let mut row_had_pii = false;

            let scrubbed_row: Vec<String> = record
                .iter()
                .enumerate()
                .map(|(cell_idx, cell)| {
                    let mut matches = all_regex[i][cell_idx].clone();

                    // Map NER matches from joined string back to this cell
                    if ner_enabled && cell_idx < all_offsets[i].len() {
                        let (cell_start, cell_end) = all_offsets[i][cell_idx];
                        for nm in &ner_results[i] {
                            if nm.start >= cell_start && nm.end <= cell_end {
                                matches.push(PiiMatch {
                                    pii_type: nm.pii_type.clone(),
                                    value: nm.value.clone(),
                                    start: nm.start - cell_start,
                                    end: nm.end - cell_start,
                                });
                            }
                        }
                    }

                    matches.sort_by_key(|m| m.start);
                    let matches = PiiDetector::dedup_overlapping(matches);

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

            writer.write_record(&scrubbed_row)?;
            stats.rows_processed += 1;
        }

        Ok(())
    }

    /// Process a text file line by line
    pub fn process_text_streaming(
        &self,
        input_path: &Path,
        output_path: &Path,
        progress_tx: Option<Sender<ProcessingProgress>>,
    ) -> Result<ProcessingStats, ProcessError> {
        let ner_enabled = self.detector.uses_ner() && is_ner_available();

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
        let mut line_buffer: Vec<String> = Vec::with_capacity(NER_BATCH_SIZE);
        let mut line_bytes: u64 = 0;

        for line_result in reader.lines() {
            let line = line_result?;
            line_bytes += line.len() as u64 + 1;
            line_buffer.push(line);

            if line_buffer.len() >= NER_BATCH_SIZE {
                bytes_processed += line_bytes;
                self.scrub_text_batch(&line_buffer, &mut writer, &mut stats, ner_enabled)?;

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

                line_buffer.clear();
                line_bytes = 0;
            }
        }

        // Flush remaining lines
        if !line_buffer.is_empty() {
            bytes_processed += line_bytes;
            self.scrub_text_batch(&line_buffer, &mut writer, &mut stats, ner_enabled)?;
        }

        writer.flush()?;

        Ok(stats)
    }

    /// Micro-batch scrub for text lines: regex each line, batch NER, merge + write.
    fn scrub_text_batch(
        &self,
        lines: &[String],
        writer: &mut BufWriter<File>,
        stats: &mut ProcessingStats,
        ner_enabled: bool,
    ) -> Result<(), ProcessError> {
        // Regex per line
        let regex_matches: Vec<Vec<PiiMatch>> = lines
            .iter()
            .map(|line| self.detector.detect_regex(line))
            .collect();

        // Batch NER
        let ner_matches = if ner_enabled {
            let refs: Vec<&str> = lines.iter().map(|s| s.as_str()).collect();
            self.detector.detect_ner(&refs)
        } else {
            vec![Vec::new(); lines.len()]
        };

        // Merge + scrub + write
        for (i, line) in lines.iter().enumerate() {
            let mut all = regex_matches[i].clone();
            all.extend(ner_matches[i].clone());
            all.sort_by_key(|m| m.start);
            let all = PiiDetector::dedup_overlapping(all);

            if !all.is_empty() {
                stats.total_pii_found += all.len();
                stats.rows_affected += 1;
            }

            let scrubbed = self.scrubber.scrub_text(line, &all);
            writeln!(writer, "{}", scrubbed)?;
            stats.rows_processed += 1;
        }

        Ok(())
    }

    /// Process bytes directly (writes to temp, processes, reads back)
    /// Returns the scrubbed content as bytes
    pub fn process_bytes(
        &self,
        input_bytes: &[u8],
        file_extension: &str,
        progress_tx: Option<Sender<ProcessingProgress>>,
    ) -> Result<(Vec<u8>, ProcessingStats), ProcessError> {
        // Write input to a temp file
        let input_temp = TempFile::new(file_extension)?;
        {
            let mut writer = BufWriter::new(File::create(input_temp.path())?);
            writer.write_all(input_bytes)?;
            writer.flush()?;
        }

        // Create a separate temp file for output
        let output_temp = TempFile::new(file_extension)?;

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

        // Read processed output
        let output_file = File::open(output_temp.path())?;
        let size = output_file.metadata()?.len() as usize;

        let mut reader = BufReader::with_capacity(64 * 1024, output_file);
        let mut output_bytes = Vec::with_capacity(size);
        reader.read_to_end(&mut output_bytes)?;

        // TempFiles clean up automatically on drop
        Ok((output_bytes, stats))
    }
}
