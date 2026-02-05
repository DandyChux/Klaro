use crate::file_parser::{FileContent, FileParser, FileType, TabularData, content_to_csv};
use crate::license::{LimitError, VersionLimits};
use crate::pii_detector::{PiiDetector, PiiMatch, PiiType};
use crate::scrubber::{ScrubConfig, ScrubMethod, Scrubber};
use crate::streaming::{ProcessingProgress, ProcessingStats, StreamingProcessor};
use base64::{Engine, engine::general_purpose::STANDARD};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::mpsc;
use tauri::{Runtime, Window};

#[derive(Debug, Serialize, Deserialize)]
pub struct ProcessRequest {
    pub file_data: String, // Base64 encoded file data
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
}

#[derive(Debug, Serialize, Deserialize, Default)]
pub struct ScrubStats {
    pub total_pii_found: usize,
    pub pii_by_type: HashMap<String, usize>,
    pub rows_affected: usize,
    pub cells_affected: usize,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct PiiTypeInfo {
    pub id: String,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct SupportedFormat {
    pub extension: String,
    pub name: String,
    pub mime_type: String,
}

fn parse_pii_types(types: &[String]) -> Vec<PiiType> {
    types
        .iter()
        .filter_map(|t| match t.to_lowercase().as_str() {
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
            _ => None,
        })
        .collect()
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

#[tauri::command]
pub fn process_file(request: ProcessRequest) -> ProcessResult {
    // Decode base64 file data
    let file_bytes = match STANDARD.decode(&request.file_data) {
        Ok(bytes) => bytes,
        Err(e) => {
            return ProcessResult {
                success: false,
                scrubbed_data: None,
                scrubbed_preview: None,
                file_name: request.file_name,
                file_type: "unknown".to_string(),
                stats: ScrubStats::default(),
                error: Some(format!("Failed to decode file data: {}", e)),
            };
        }
    };

    // Parse the file
    let parsed = match FileParser::parse_from_bytes(&file_bytes, &request.file_name) {
        Ok(p) => p,
        Err(e) => {
            return ProcessResult {
                success: false,
                scrubbed_data: None,
                scrubbed_preview: None,
                file_name: request.file_name,
                file_type: "unknown".to_string(),
                stats: ScrubStats::default(),
                error: Some(format!("Failed to parse file: {}", e)),
            };
        }
    };

    // Setup PII detector
    let pii_types = if request.pii_types.is_empty() {
        PiiType::all()
    } else {
        parse_pii_types(&request.pii_types)
    };
    let detector = PiiDetector::new(pii_types);

    // Setup scrubber
    let scrub_config = ScrubConfig {
        method: parse_scrub_method(&request.scrub_method),
        preserve_format: true,
    };
    let scrubber = Scrubber::new(scrub_config);

    // Process the content
    let (scrubbed_content, stats) = match &parsed.content {
        FileContent::Tabular(data) => process_tabular(&detector, &scrubber, data),
        FileContent::Text(text) => process_text(&detector, &scrubber, text),
    };

    // Convert back to file format
    let output_data = match content_to_csv(&scrubbed_content) {
        Ok(data) => data,
        Err(e) => {
            return ProcessResult {
                success: false,
                scrubbed_data: None,
                scrubbed_preview: None,
                file_name: request.file_name,
                file_type: parsed.file_type.extension().to_string(),
                stats,
                error: Some(format!("Failed to generate output: {}", e)),
            };
        }
    };

    // Generate preview (first 1000 chars)
    let preview = if output_data.len() > 1000 {
        format!(
            "{}...\n\n[Truncated - {} total characters]",
            &output_data[..1000],
            output_data.len()
        )
    } else {
        output_data.clone()
    };

    // Encode output as base64
    let encoded_output = STANDARD.encode(output_data.as_bytes());

    // Generate output filename
    let output_name = generate_output_filename(&request.file_name);

    ProcessResult {
        success: true,
        scrubbed_data: Some(encoded_output),
        scrubbed_preview: Some(preview),
        file_name: output_name,
        file_type: parsed.file_type.extension().to_string(),
        stats,
        error: None,
    }
}

fn process_tabular(
    detector: &PiiDetector,
    scrubber: &Scrubber,
    data: &TabularData,
) -> (FileContent, ScrubStats) {
    let mut stats = ScrubStats::default();
    let mut pii_counts: HashMap<String, usize> = HashMap::new();

    // Process headers
    let scrubbed_headers = data.headers.as_ref().map(|headers| {
        headers
            .iter()
            .map(|h| {
                let matches = detector.detect(h);
                update_stats(&matches, &mut stats, &mut pii_counts);
                scrubber.scrub_text(h, &matches)
            })
            .collect()
    });

    // Process rows
    let mut rows_affected_set = std::collections::HashSet::new();
    let scrubbed_rows: Vec<Vec<String>> = data
        .rows
        .iter()
        .enumerate()
        .map(|(row_idx, row)| {
            row.iter()
                .map(|cell| {
                    let matches = detector.detect(cell);
                    if !matches.is_empty() {
                        rows_affected_set.insert(row_idx);
                        stats.cells_affected += 1;
                    }
                    update_stats(&matches, &mut stats, &mut pii_counts);
                    scrubber.scrub_text(cell, &matches)
                })
                .collect()
        })
        .collect();

    stats.rows_affected = rows_affected_set.len();
    stats.pii_by_type = pii_counts;

    let scrubbed_data = TabularData {
        headers: scrubbed_headers,
        rows: scrubbed_rows,
        sheet_name: data.sheet_name.clone(),
    };

    (FileContent::Tabular(scrubbed_data), stats)
}

fn process_text(
    detector: &PiiDetector,
    scrubber: &Scrubber,
    text: &str,
) -> (FileContent, ScrubStats) {
    let mut stats = ScrubStats::default();
    let mut pii_counts: HashMap<String, usize> = HashMap::new();

    let matches = detector.detect(text);
    update_stats(&matches, &mut stats, &mut pii_counts);

    if !matches.is_empty() {
        stats.rows_affected = 1;
        stats.cells_affected = 1;
    }

    stats.pii_by_type = pii_counts;
    let scrubbed = scrubber.scrub_text(text, &matches);

    (FileContent::Text(scrubbed), stats)
}

fn update_stats(
    matches: &[PiiMatch],
    stats: &mut ScrubStats,
    pii_counts: &mut HashMap<String, usize>,
) {
    stats.total_pii_found += matches.len();
    for m in matches {
        let type_name = m.pii_type.display_name().to_string();
        *pii_counts.entry(type_name).or_insert(0) += 1;
    }
}

fn generate_output_filename(original: &str) -> String {
    let path = std::path::Path::new(original);
    let stem = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("output");
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("csv");

    // For Excel files, output as CSV since we don't have xlsx writing
    let binding = ext.to_lowercase();
    let output_ext = match binding.as_str() {
        "xlsx" | "xls" => "csv",
        other => other,
    };

    format!("{}_scrubbed.{}", stem, output_ext)
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
    vec![
        PiiTypeInfo {
            id: "email".to_string(),
            name: "Email Address".to_string(),
            description: "Email addresses (e.g., john.doe@example.com)".to_string(),
        },
        PiiTypeInfo {
            id: "phone".to_string(),
            name: "Phone Number".to_string(),
            description: "Phone numbers in various formats".to_string(),
        },
        PiiTypeInfo {
            id: "ssn".to_string(),
            name: "Social Security Number".to_string(),
            description: "US Social Security Numbers (XXX-XX-XXXX)".to_string(),
        },
        PiiTypeInfo {
            id: "credit_card".to_string(),
            name: "Credit Card Number".to_string(),
            description: "Credit/debit card numbers (Visa, MasterCard, Amex, etc.)".to_string(),
        },
        PiiTypeInfo {
            id: "ip_address".to_string(),
            name: "IP Address".to_string(),
            description: "IPv4 addresses".to_string(),
        },
        PiiTypeInfo {
            id: "date_of_birth".to_string(),
            name: "Date of Birth".to_string(),
            description: "Dates that may represent birth dates".to_string(),
        },
        PiiTypeInfo {
            id: "address".to_string(),
            name: "Street Address".to_string(),
            description: "Physical street addresses".to_string(),
        },
        PiiTypeInfo {
            id: "name".to_string(),
            name: "Person Name".to_string(),
            description: "Names with common titles (Mr., Mrs., Dr., etc.)".to_string(),
        },
        PiiTypeInfo {
            id: "passport".to_string(),
            name: "Passport Number".to_string(),
            description: "Passport identification numbers".to_string(),
        },
        PiiTypeInfo {
            id: "drivers_license".to_string(),
            name: "Driver's License".to_string(),
            description: "Driver's license numbers".to_string(),
        },
        PiiTypeInfo {
            id: "bank_account".to_string(),
            name: "Bank Account".to_string(),
            description: "Bank account numbers".to_string(),
        },
    ]
}
