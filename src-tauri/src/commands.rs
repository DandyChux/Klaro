use crate::file_parser::{FileContent, FileParser, FileType, TabularData, content_to_csv};
use crate::license::VersionLimits;
use crate::pii_detector::{PiiDetector, PiiMatch, PiiType};
use crate::scrubber::{ScrubConfig, ScrubMethod, Scrubber};
use crate::streaming::{ProcessingProgress, ProcessingStats as StreamingStats, StreamingProcessor};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc;
use tauri::Emitter;

// Threshold for using streaming vs in-memory processing (10 MB)
const STREAMING_THRESHOLD: usize = 10 * 1024 * 1024;

// Global cancellation flag - set by cancel_processing, checked during row loops
static CANCEL_FLAG: AtomicBool = AtomicBool::new(false);

// ============================================================================
// Request/Response Types
// ============================================================================

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessRequest {
    pub file_data: String, // Base64 encoded
    pub file_name: String,
    pub pii_types: Vec<String>,
    pub scrub_method: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessResult {
    pub success: bool,
    pub scrubbed_data: Option<String>, // Base64 encoded
    pub scrubbed_preview: Option<String>,
    pub file_name: String,
    pub file_type: String,
    pub stats: ScrubStats,
    pub error: Option<String>,
    pub limit_warning: Option<String>,
    pub used_streaming: bool,
}

#[derive(Debug, Serialize, Deserialize, Default, Clone)]
pub struct ScrubStats {
    pub total_pii_found: usize,
    pub pii_by_type: HashMap<String, usize>,
    pub rows_affected: usize,
    pub rows_processed: usize,
    pub cells_affected: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PiiTypeInfo {
    pub id: String,
    pub name: String,
    pub description: String,
    pub available_in_lite: bool,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SupportedFormat {
    pub extension: String,
    pub name: String,
    pub mime_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppVersionInfo {
    pub version_name: String,
    pub is_trial: bool,
    pub limits: VersionLimits,
    pub features: VersionFeatures,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionFeatures {
    pub max_file_size_display: String,
    pub max_rows_display: String,
    pub max_files_display: String,
    pub available_pii_types: Vec<String>,
    pub all_pii_types: Vec<String>,
}

/// Payload emitted to the frontend via Tauri events during processing
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ProgressPayload {
    pub phase: String,
    pub rows_processed: usize,
    pub total_rows: usize,
    pub pii_found: usize,
    pub percent: f32,
}

// ============================================================================
// Helper Functions
// ============================================================================

fn is_cancelled() -> bool {
    CANCEL_FLAG.load(Ordering::Relaxed)
}

fn parse_pii_types(types: &[String], limits: &VersionLimits) -> Result<Vec<PiiType>, String> {
    let available_pii_types = limits
        .allowed_pii_types
        .as_ref()
        .map(|v| v.clone())
        .unwrap_or_default();
    let mut result = Vec::new();

    for t in types {
        let pii_type = match t.to_lowercase().as_str() {
            "email" => Some(PiiType::Email),
            "phone" => Some(PiiType::Phone),
            "ssn" => Some(PiiType::SSN),
            "creditcard" | "credit_card" => Some(PiiType::CreditCard),
            "ipaddress" | "ip_address" | "ip" => Some(PiiType::IPAddress),
            "dateofbirth" | "date_of_birth" | "dob" => Some(PiiType::DateOfBirth),
            "address" => Some(PiiType::Address),
            "name" => Some(PiiType::Name),
            "passport" => Some(PiiType::Passport),
            "driverslicense" | "drivers_license" => Some(PiiType::DriversLicense),
            "bankaccount" | "bank_account" => Some(PiiType::BankAccount),
            "organization" | "org" => Some(PiiType::Organization),
            _ => None,
        };

        if let Some(pii) = pii_type {
            // Check if allowed in current version
            if limits.is_trial && !available_pii_types.contains(&t) {
                return Err(format!(
                    "PII type '{}' is not available in Lite version. Upgrade to Pro for all PII types.",
                    t
                ));
            }
            result.push(pii);
        }
    }

    Ok(result)
}

fn parse_scrub_method(method: &str) -> ScrubMethod {
    match method.to_lowercase().as_str() {
        "remove" => ScrubMethod::Remove,
        "hash" => ScrubMethod::Hash,
        "fake" => ScrubMethod::Fake,
        "mask" => ScrubMethod::Mask,
        _ => ScrubMethod::Mask,
    }
}

fn generate_output_filename(original: &str) -> String {
    let path = std::path::Path::new(original);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("csv");

    let lowercase_ext = ext.to_lowercase();
    let output_ext = match lowercase_ext.as_str() {
        "xlsx" | "xls" => "csv",
        other => other,
    };

    format!("{}_scrubbed.{}", stem, output_ext)
}

fn get_file_extension(filename: &str) -> String {
    std::path::Path::new(filename)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("txt")
        .to_lowercase()
}

fn process_tabular(
    detector: &PiiDetector,
    scrubber: &Scrubber,
    data: &TabularData,
    app_handle: Option<&tauri::AppHandle>,
) -> Result<(FileContent, ScrubStats), String> {
    let total_rows = data.rows.len();
    let ner_enabled = detector.uses_ner();
    let separator = " | ";

    let num_workers = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
        .min(8);

    let mut stats = ScrubStats::default();
    let mut pii_counts: HashMap<String, usize> = HashMap::new();

    // Process headers (regex only — headers aren't PII names/addresses)
    let scrubbed_headers = data.headers.as_ref().map(|headers| {
        headers
            .iter()
            .map(|h| {
                let matches = detector.detect_regex(h);
                update_stats(&matches, &mut stats, &mut pii_counts);
                scrubber.scrub_text(h, &matches)
            })
            .collect()
    });

    if total_rows == 0 {
        stats.pii_by_type = pii_counts;
        return Ok((
            FileContent::Tabular(TabularData {
                headers: scrubbed_headers,
                rows: Vec::new(),
                sheet_name: data.sheet_name.clone(),
            }),
            stats,
        ));
    }

    // ================================================================
    // Phase 1: Parallel regex detection + NER row preparation
    // ================================================================

    struct RowPrep {
        cell_regex_matches: Vec<Vec<PiiMatch>>,
        joined_string: String,
        cell_offsets: Vec<(usize, usize)>,
    }

    let chunk_size = (total_rows + num_workers - 1) / num_workers;

    let row_preps: Vec<RowPrep> = std::thread::scope(|s| {
        let (tx, rx) = mpsc::channel::<(usize, Vec<RowPrep>)>();

        for (chunk_idx, chunk) in data.rows.chunks(chunk_size).enumerate() {
            let tx = tx.clone();
            s.spawn(move || {
                let mut preps = Vec::with_capacity(chunk.len());
                for row in chunk {
                    let cell_regex_matches: Vec<Vec<PiiMatch>> =
                        row.iter().map(|cell| detector.detect_regex(cell)).collect();

                    let mut joined = String::new();
                    let mut cell_offsets = Vec::with_capacity(row.len());
                    if ner_enabled {
                        for (idx, cell) in row.iter().enumerate() {
                            if idx > 0 {
                                joined.push_str(separator)
                            }
                            let start = joined.len();
                            joined.push_str(cell);
                            cell_offsets.push((start, joined.len()));
                        }
                    }

                    preps.push(RowPrep {
                        cell_regex_matches,
                        joined_string: joined,
                        cell_offsets,
                    });
                }
                let _ = tx.send((chunk_idx, preps));
            });
        }
        drop(tx);

        let mut chunks: Vec<(usize, Vec<RowPrep>)> = rx.iter().collect();
        chunks.sort_by_key(|(idx, _)| *idx);
        chunks.into_iter().flat_map(|(_, preps)| preps).collect()
    });

    if is_cancelled() {
        return Err("Processing cancelled".to_string());
    }

    if let Some(handle) = app_handle {
        let _ = handle.emit(
            "processing-progress",
            ProgressPayload {
                phase: "regex_complete".to_string(),
                rows_processed: total_rows,
                total_rows,
                pii_found: 0,
                percent: 25.0,
            },
        );
    }

    // ================================================================
    // Phase 2: Batched NER inference
    // ================================================================

    let ner_results: Vec<Vec<PiiMatch>> = if ner_enabled {
        let joined_refs: Vec<&str> = row_preps
            .iter()
            .map(|rp| rp.joined_string.as_str())
            .collect();

        // Process in progress-friendly chunks; detect_ner handles
        // sub-batching to the model's optimal batch size internally
        const NER_PROGRESS_CHUNK: usize = 256;
        let mut all_ner: Vec<Vec<PiiMatch>> = Vec::with_capacity(total_rows);

        for chunk in joined_refs.chunks(NER_PROGRESS_CHUNK) {
            if is_cancelled() {
                return Err("Processing cancelled".to_string());
            }

            all_ner.extend(detector.detect_ner(chunk));

            if let Some(handle) = app_handle {
                let done = all_ner.len();
                let percent = 25.0 + (done as f32 / total_rows as f32) * 55.0;
                let _ = handle.emit(
                    "processing-progress",
                    ProgressPayload {
                        phase: "ner_scanning".to_string(),
                        rows_processed: done,
                        total_rows,
                        pii_found: 0,
                        percent,
                    },
                );
            }
        }
        all_ner
    } else {
        vec![Vec::new(); total_rows]
    };

    if is_cancelled() {
        return Err("Processing cancelled".to_string());
    }

    // ================================================================
    // Phase 3: Parallel merge + scrub (threaded with channels)
    // ================================================================

    struct ChunkResult {
        chunk_idx: usize,
        scrubbed_rows: Vec<Vec<String>>,
        total_pii_found: usize,
        pii_counts: HashMap<String, usize>,
        rows_affected: usize,
        cells_affected: usize,
    }

    let scrub_results: Vec<ChunkResult> = std::thread::scope(|s| {
        let (tx, rx) = mpsc::channel::<ChunkResult>();

        let num_chunks = data.rows.chunks(chunk_size).len();
        for chunk_idx in 0..num_chunks {
            let start = chunk_idx * chunk_size;
            let end = (start + chunk_size).min(total_rows);

            let row_slice = &data.rows[start..end];
            let prep_slice = &row_preps[start..end];
            let ner_slice = &ner_results[start..end];
            let tx = tx.clone();

            s.spawn(move || {
                let mut scrubbed_rows = Vec::with_capacity(end - start);
                let mut local_pii_found = 0usize;
                let mut local_pii_counts: HashMap<String, usize> = HashMap::new();
                let mut local_rows_affected = 0usize;
                let mut local_cells_affected = 0usize;

                for ((row, prep), ner_matches) in row_slice
                    .iter()
                    .zip(prep_slice.iter())
                    .zip(ner_slice.iter())
                {
                    // Map NER matches from joined string back to individual cells
                    let mut cell_ner: Vec<Vec<PiiMatch>> = vec![Vec::new(); row.len()];
                    if ner_enabled {
                        for nm in ner_matches {
                            for (ci, &(cs, ce)) in prep.cell_offsets.iter().enumerate() {
                                if nm.start >= cs && nm.end <= ce {
                                    cell_ner[ci].push(PiiMatch {
                                        pii_type: nm.pii_type.clone(),
                                        value: nm.value.clone(),
                                        start: nm.start - cs,
                                        end: nm.end - cs,
                                    });
                                    break;
                                }
                            }
                        }
                    }

                    let mut row_had_pii = false;
                    let scrubbed_row: Vec<String> = row
                        .iter()
                        .enumerate()
                        .map(|(ci, cell)| {
                            let mut all = prep.cell_regex_matches[ci].clone();
                            all.extend(cell_ner[ci].clone());
                            all.sort_by_key(|m| m.start);
                            let all = PiiDetector::dedup_overlapping(all);

                            if !all.is_empty() {
                                row_had_pii = true;
                                local_cells_affected += 1;
                            }
                            local_pii_found += all.len();
                            for m in &all {
                                *local_pii_counts
                                    .entry(m.pii_type.display_name().to_string())
                                    .or_insert(0) += 1;
                            }
                            scrubber.scrub_text(cell, &all)
                        })
                        .collect();

                    if row_had_pii {
                        local_rows_affected += 1;
                    }
                    scrubbed_rows.push(scrubbed_row);
                }

                let _ = tx.send(ChunkResult {
                    chunk_idx,
                    scrubbed_rows,
                    total_pii_found: local_pii_found,
                    pii_counts: local_pii_counts,
                    rows_affected: local_rows_affected,
                    cells_affected: local_cells_affected,
                });
            });
        }
        drop(tx);

        let mut results: Vec<ChunkResult> = rx.iter().collect();
        results.sort_by_key(|r| r.chunk_idx);
        results
    });

    // Reassemble results in order
    let mut scrubbed_rows: Vec<Vec<String>> = Vec::with_capacity(total_rows);
    for chunk in scrub_results {
        scrubbed_rows.extend(chunk.scrubbed_rows);
        stats.total_pii_found += chunk.total_pii_found;
        stats.rows_affected += chunk.rows_affected;
        for (k, v) in chunk.pii_counts {
            *pii_counts.entry(k).or_insert(0) += v;
        }
    }

    stats.rows_processed = total_rows;
    stats.pii_by_type = pii_counts;

    if let Some(handle) = app_handle {
        let _ = handle.emit(
            "processing-progress",
            ProgressPayload {
                phase: "scrubbing".to_string(),
                rows_processed: total_rows,
                total_rows,
                pii_found: stats.total_pii_found,
                percent: 100.0,
            },
        );
    }

    Ok((
        FileContent::Tabular(TabularData {
            headers: scrubbed_headers,
            rows: scrubbed_rows,
            sheet_name: data.sheet_name.clone(),
        }),
        stats,
    ))
}

fn process_text(
    detector: &PiiDetector,
    scrubber: &Scrubber,
    text: &str,
    app_handle: Option<&tauri::AppHandle>,
) -> Result<(FileContent, ScrubStats), String> {
    if is_cancelled() {
        return Err("Processing cancelled".to_string());
    }

    let mut stats = ScrubStats::default();
    let mut pii_counts: HashMap<String, usize> = HashMap::new();
    let ner_enabled = detector.uses_ner();

    // Emit initial progress
    if let Some(handle) = app_handle {
        let _ = handle.emit(
            "processing-progress",
            ProgressPayload {
                phase: "scanning".to_string(),
                rows_processed: 0,
                total_rows: 1,
                pii_found: 0,
                percent: 10.0,
            },
        );
    }

    // Phase 1: Regex detection
    let mut matches = detector.detect_regex(text);

    if is_cancelled() {
        return Err("Processing cancelled".to_string());
    }

    if let Some(handle) = app_handle {
        let _ = handle.emit(
            "processing-progress",
            ProgressPayload {
                phase: "regex_complete".to_string(),
                rows_processed: 1,
                total_rows: 1,
                pii_found: matches.len(),
                percent: 40.0,
            },
        );
    }

    // Phase 2: NER detection (if enabled)
    if ner_enabled {
        if let Some(handle) = app_handle {
            let _ = handle.emit(
                "processing-progress",
                ProgressPayload {
                    phase: "ner_scanning".to_string(),
                    rows_processed: 0,
                    total_rows: 1,
                    pii_found: matches.len(),
                    percent: 50.0,
                },
            );
        }

        let ner_results = detector.detect_ner(&[text]);
        if !ner_results.is_empty() {
            matches.extend(ner_results.into_iter().next().unwrap_or_default());
        }

        // Sort and deduplicate overlapping matches
        matches.sort_by_key(|m| m.start);
        matches = PiiDetector::dedup_overlapping(matches);
    }

    if is_cancelled() {
        return Err("Processing cancelled".to_string());
    }

    // Phase 3: Update stats and scrub
    update_stats(&matches, &mut stats, &mut pii_counts);

    if !matches.is_empty() {
        stats.rows_affected = 1;
        stats.cells_affected = 1;
    }

    stats.rows_processed = 1;
    stats.pii_by_type = pii_counts;

    let scrubbed_text = scrubber.scrub_text(text, &matches);

    if let Some(handle) = app_handle {
        let _ = handle.emit(
            "processing-progress",
            ProgressPayload {
                phase: "scrubbing".to_string(),
                rows_processed: 1,
                total_rows: 1,
                pii_found: stats.total_pii_found,
                percent: 100.0,
            },
        );
    }

    Ok((FileContent::Text(scrubbed_text), stats))
}

fn update_stats(
    matches: &[PiiMatch],
    stats: &mut ScrubStats,
    pii_counts: &mut HashMap<String, usize>,
) {
    stats.total_pii_found += matches.len();
    for m in matches {
        *pii_counts
            .entry(m.pii_type.display_name().to_string())
            .or_insert(0) += 1;
    }
}

fn make_error_result(file_name: String, file_type: &str, error: String) -> ProcessResult {
    ProcessResult {
        success: false,
        scrubbed_data: None,
        scrubbed_preview: None,
        file_name,
        file_type: file_type.to_string(),
        stats: ScrubStats::default(),
        error: Some(error),
        limit_warning: None,
        used_streaming: false,
    }
}

// ============================================================================
// Tauri Commands
// ============================================================================

#[tauri::command]
pub fn cancel_processing() {
    CANCEL_FLAG.store(true, Ordering::Relaxed);
}

#[tauri::command]
pub fn get_version_info() -> AppVersionInfo {
    let limits = VersionLimits::current();
    let all_types = vec![
        "email",
        "phone",
        "ssn",
        "credit_card",
        "ip_address",
        "date_of_birth",
        "address",
        "name",
        "passport",
        "drivers_license",
        "bank_account",
    ];

    let available_pii_types = limits
        .allowed_pii_types
        .as_ref()
        .map(|v| v.clone())
        .unwrap_or_default();

    let features = VersionFeatures {
        max_file_size_display: match limits.max_file_size_bytes {
            Some(bytes) => format!("{} MB", bytes / (1024 * 1024)),
            None => "Unlimited".to_string(),
        },
        max_rows_display: match limits.max_rows_per_file {
            Some(rows) => format!("{} rows", rows),
            None => "Unlimited".to_string(),
        },
        max_files_display: match limits.max_files_per_batch {
            Some(files) => format!("{} files", files),
            None => "Unlimited".to_string(),
        },
        available_pii_types,
        all_pii_types: all_types.iter().map(|s| s.to_string()).collect(),
    };

    AppVersionInfo {
        version_name: limits.version_name.clone(),
        is_trial: limits.is_trial,
        limits,
        features,
    }
}

#[tauri::command]
pub fn validate_files(file_sizes: Vec<u64>, file_count: usize) -> Result<(), String> {
    let limits = VersionLimits::current();
    limits
        .check_batch_size(file_count)
        .map_err(|e| e.to_string())?;
    for size in file_sizes {
        limits.check_file_size(size).map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn process_file(app_handle: tauri::AppHandle, request: ProcessRequest) -> ProcessResult {
    // Reset cancellation flag at the start of each run
    CANCEL_FLAG.store(false, Ordering::Relaxed);

    let limits = VersionLimits::current();

    // Emit initial progress event
    let _ = app_handle.emit(
        "processing-progress",
        ProgressPayload {
            phase: "preparing".to_string(),
            rows_processed: 0,
            total_rows: 0,
            pii_found: 0,
            percent: 0.0,
        },
    );

    let result = tauri::async_runtime::spawn_blocking(move || {
        // Decode base64 file data
        let file_bytes = match STANDARD.decode(&request.file_data) {
            Ok(bytes) => bytes,
            Err(e) => {
                return make_error_result(
                    request.file_name,
                    "unknown",
                    format!("Failed to decode file data: {}", e),
                );
            }
        };

        // Check file size limit
        if let Err(e) = limits.check_file_size(file_bytes.len() as u64) {
            return make_error_result(request.file_name, "unknown", e.to_string());
        }

        // Setup PII detector with limit checks
        let pii_types = match parse_pii_types(&request.pii_types, &limits) {
            Ok(types) if types.is_empty() => PiiType::all(),
            Ok(types) => types,
            Err(e) => return make_error_result(request.file_name, "unknown", e),
        };

        let detector = PiiDetector::new(pii_types);
        let scrubber = Scrubber::new(ScrubConfig {
            method: parse_scrub_method(&request.scrub_method),
            preserve_format: true,
        });

        let file_extension = get_file_extension(&request.file_name);
        let use_streaming = file_bytes.len() > STREAMING_THRESHOLD
            && (file_extension == "csv" || file_extension == "txt");

        // Process using appropriate method
        if use_streaming {
            process_file_streaming(
                &app_handle,
                request,
                file_bytes,
                detector,
                scrubber,
                limits,
                file_extension,
            )
        } else {
            process_file_in_memory(&app_handle, request, file_bytes, detector, scrubber, limits)
        }
    })
    .await
    .unwrap_or_else(|e| {
        make_error_result(
            "unknown".to_string(),
            "unknown",
            format!("Processing thread panicked: {}", e),
        )
    });

    result
}

fn process_file_streaming(
    app_handle: &tauri::AppHandle,
    request: ProcessRequest,
    file_bytes: Vec<u8>,
    detector: PiiDetector,
    scrubber: Scrubber,
    limits: VersionLimits,
    file_extension: String,
) -> ProcessResult {
    let processor = StreamingProcessor::new(detector, scrubber, limits.clone());

    let (progress_tx, progress_rx) = mpsc::channel::<ProcessingProgress>();
    let progress_handle = app_handle.clone();
    std::thread::spawn(move || {
        while let Ok(progress) = progress_rx.recv() {
            let _ = progress_handle.emit(
                "processing-progress",
                ProgressPayload {
                    phase: progress.current_phase,
                    rows_processed: progress.rows_processed,
                    total_rows: 0,
                    pii_found: progress.pii_found,
                    percent: progress.percent_complete,
                },
            );
        }
    });

    match processor.process_bytes(&file_bytes, &file_extension, Some(progress_tx)) {
        Ok((output_bytes, stats)) => {
            if is_cancelled() {
                return make_error_result(
                    request.file_name,
                    &file_extension,
                    "Processing cancelled".to_string(),
                );
            }

            // Generate preview from first 2000 bytes
            let preview_bytes = if output_bytes.len() > 2000 {
                &output_bytes[..2000]
            } else {
                &output_bytes
            };
            let preview = String::from_utf8_lossy(preview_bytes).to_string();
            let preview = if output_bytes.len() > 2000 {
                format!(
                    "{}...\n\n[Truncated - {} total bytes]",
                    preview,
                    output_bytes.len()
                )
            } else {
                preview
            };

            ProcessResult {
                success: true,
                scrubbed_data: Some(STANDARD.encode(&output_bytes)),
                scrubbed_preview: Some(preview),
                file_name: generate_output_filename(&request.file_name),
                file_type: file_extension,
                stats: ScrubStats {
                    total_pii_found: stats.total_pii_found,
                    pii_by_type: HashMap::new(), // Streaming doesn't track by type currently
                    rows_affected: stats.rows_affected,
                    rows_processed: stats.rows_processed,
                    cells_affected: stats.cells_affected,
                },
                error: None,
                limit_warning: if limits.is_trial {
                    Some("You're using Klaro Lite. Upgrade to Pro for unlimited file sizes and features.".to_string())
                } else {
                    None
                },
                used_streaming: true,
            }
        }
        Err(e) => make_error_result(request.file_name, &file_extension, e.to_string()),
    }
}

/// Process smaller files in memory (faster for small files)
fn process_file_in_memory(
    app_handle: &tauri::AppHandle,
    request: ProcessRequest,
    file_bytes: Vec<u8>,
    detector: PiiDetector,
    scrubber: Scrubber,
    limits: VersionLimits,
) -> ProcessResult {
    // Parse the file
    let parsed = match FileParser::parse_from_bytes(&file_bytes, &request.file_name) {
        Ok(p) => p,
        Err(e) => {
            return make_error_result(
                request.file_name,
                "unknown",
                format!("Failed to parse file: {}", e),
            );
        }
    };

    // Check row limits for tabular data
    if let FileContent::Tabular(ref data) = parsed.content {
        if let Err(e) = limits.check_row_count(data.rows.len()) {
            return make_error_result(
                request.file_name,
                &parsed.file_type.extension(),
                e.to_string(),
            );
        }
    }

    // Process content
    let process_result = match &parsed.content {
        FileContent::Tabular(data) => process_tabular(&detector, &scrubber, data, Some(app_handle)),
        FileContent::Text(text) => process_text(&detector, &scrubber, text, Some(app_handle)),
    };

    let (scrubbed_content, stats) = match process_result {
        Ok(r) => r,
        Err(e) => {
            return make_error_result(request.file_name, &parsed.file_type.extension(), e);
        }
    };

    // Convert to output format
    let output_data = match content_to_csv(&scrubbed_content) {
        Ok(data) => data,
        Err(e) => {
            return make_error_result(
                request.file_name,
                &parsed.file_type.extension(),
                format!("Failed to generate output: {}", e),
            );
        }
    };

    // Generate preview
    let preview = if output_data.len() > 2000 {
        format!(
            "{}...\n\n[Truncated - {} total characters]",
            &output_data[..2000],
            output_data.len()
        )
    } else {
        output_data.clone()
    };

    ProcessResult {
        success: true,
        scrubbed_data: Some(STANDARD.encode(output_data.as_bytes())),
        scrubbed_preview: Some(preview),
        file_name: generate_output_filename(&request.file_name),
        file_type: parsed.file_type.extension().to_string(),
        stats,
        error: None,
        limit_warning: if limits.is_trial {
            Some(
                "You're using Klaro Lite. Upgrade to Pro for unlimited file sizes and features."
                    .to_string(),
            )
        } else {
            None
        },
        used_streaming: false,
    }
}

#[tauri::command]
pub fn get_supported_formats() -> Vec<SupportedFormat> {
    vec![
        SupportedFormat {
            extension: "csv".to_string(),
            name: "CSV (Comma-Separated Values)".to_string(),
            mime_type: "text/csv".to_string(),
        },
        SupportedFormat {
            extension: "xlsx".to_string(),
            name: "Excel Workbook".to_string(),
            mime_type: "application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"
                .to_string(),
        },
        SupportedFormat {
            extension: "xls".to_string(),
            name: "Excel 97-2003 Workbook".to_string(),
            mime_type: "application/vnd.ms-excel".to_string(),
        },
        SupportedFormat {
            extension: "txt".to_string(),
            name: "Plain Text".to_string(),
            mime_type: "text/plain".to_string(),
        },
        SupportedFormat {
            extension: "json".to_string(),
            name: "JSON".to_string(),
            mime_type: "application/json".to_string(),
        },
    ]
}

#[tauri::command]
pub fn get_pii_types() -> Vec<PiiTypeInfo> {
    let limits = VersionLimits::current();
    let available_pii_types = limits
        .allowed_pii_types
        .as_ref()
        .map(|v| v.clone())
        .unwrap_or_default();

    vec![
        (
            "email",
            "Email Address",
            "Email addresses (e.g., john.doe@example.com)",
        ),
        ("phone", "Phone Number", "Phone numbers in various formats"),
        (
            "ssn",
            "Social Security Number",
            "US Social Security Numbers (XXX-XX-XXXX)",
        ),
        (
            "credit_card",
            "Credit Card Number",
            "Credit/debit card numbers (Visa, MasterCard, Amex, etc.)",
        ),
        ("ip_address", "IP Address", "IPv4 addresses"),
        (
            "date_of_birth",
            "Date of Birth",
            "Dates that may represent birth dates",
        ),
        ("address", "Street Address", "Physical street addresses"),
        (
            "name",
            "Person Name",
            "Names with common titles (Mr., Mrs., Dr., etc.)",
        ),
        (
            "passport",
            "Passport Number",
            "Passport identification numbers",
        ),
        (
            "drivers_license",
            "Driver's License",
            "Driver's license numbers",
        ),
        ("bank_account", "Bank Account", "Bank account numbers"),
    ]
    .into_iter()
    .map(|(id, name, desc)| PiiTypeInfo {
        id: id.to_string(),
        name: name.to_string(),
        description: desc.to_string(),
        // Available if: it's a lite type, OR we're running Pro version
        available_in_lite: available_pii_types.contains(&id.to_string()) || !limits.is_trial,
    })
    .collect()
}
