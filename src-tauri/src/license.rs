use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VersionLimits {
    pub max_file_size_bytes: Option<u64>,
    pub max_rows_per_file: Option<usize>,
    pub max_files_per_batch: Option<usize>,
    pub max_cells_per_file: Option<usize>,
    pub allowed_pii_types: Option<Vec<String>>,
    pub allows_batch_download: bool,
    pub allows_xlsx_export: bool,
    pub version_name: String,
    pub is_trial: bool,
}

impl VersionLimits {
    #[cfg(feature = "lite")]
    pub fn current() -> Self {
        Self::lite()
    }

    #[cfg(feature = "pro")]
    pub fn current() -> Self {
        Self::pro()
    }

    #[cfg(not(any(feature = "lite", feature = "pro")))]
    pub fn current() -> Self {
        Self::lite() // Default to lite if no feature specified
    }

    pub fn lite() -> Self {
        Self {
            max_file_size_bytes: Some(5 * 1024 * 1024), // 5 MB
            max_rows_per_file: Some(10_000),
            max_files_per_batch: Some(3),
            max_cells_per_file: Some(100_000),
            allowed_pii_types: Some(vec![
                "email".to_string(),
                "phone".to_string(),
                "ssn".to_string(),
                "credit_card".to_string(),
            ]),
            allows_batch_download: false,
            allows_xlsx_export: false,
            version_name: "Rusty Scrubber Lite".to_string(),
            is_trial: true,
        }
    }

    pub fn pro() -> Self {
        Self {
            max_file_size_bytes: None, // Unlimited
            max_rows_per_file: None,   // Unlimited
            max_files_per_batch: None, // Unlimited
            max_cells_per_file: None,  // Unlimited
            allowed_pii_types: None,   // All types
            allows_batch_download: true,
            allows_xlsx_export: true,
            version_name: "Rusty Scrubber Pro".to_string(),
            is_trial: false,
        }
    }

    pub fn check_file_size(&self, size_bytes: u64) -> Result<(), LimitError> {
        if let Some(max) = self.max_file_size_bytes {
            if size_bytes > max {
                return Err(LimitError::FileTooLarge {
                    size: size_bytes,
                    max,
                });
            }
        }
        Ok(())
    }

    pub fn check_row_count(&self, rows: usize) -> Result<(), LimitError> {
        if let Some(max) = self.max_rows_per_file {
            if rows > max {
                return Err(LimitError::TooManyRows { count: rows, max });
            }
        }
        Ok(())
    }

    pub fn check_batch_size(&self, count: usize) -> Result<(), LimitError> {
        if let Some(max) = self.max_files_per_batch {
            if count > max {
                return Err(LimitError::TooManyFiles { count, max });
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum LimitError {
    FileTooLarge { size: u64, max: u64 },
    TooManyRows { count: usize, max: usize },
    TooManyFiles { count: usize, max: usize },
    TooManyCells { count: usize, max: usize },
    PiiTypeNotAllowed { pii_type: String },
    FeatureNotAvailable { feature: String },
}

impl std::fmt::Display for LimitError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LimitError::FileTooLarge { size, max } => {
                write!(
                    f,
                    "File size ({:.2} MB) exceeds the limit ({:.2} MB). Upgrade to Pro for unlimited file sizes.",
                    *size as f64 / 1_048_576.0,
                    *max as f64 / 1_048_576.0
                )
            }
            LimitError::TooManyRows { count, max } => {
                write!(
                    f,
                    "Row count ({}) exceeds the limit ({}). Upgrade to Pro for unlimited rows.",
                    count, max
                )
            }
            LimitError::TooManyFiles { count, max } => {
                write!(
                    f,
                    "Batch size ({} files) exceeds the limit ({} files). Upgrade to Pro for unlimited batch processing.",
                    count, max
                )
            }
            LimitError::TooManyCells { count, max } => {
                write!(
                    f,
                    "Cell count ({}) exceeds the limit ({}). Upgrade to Pro for unlimited processing.",
                    count, max
                )
            }
            LimitError::PiiTypeNotAllowed { pii_type } => {
                write!(
                    f,
                    "PII type '{}' is not available in Lite version. Upgrade to Pro for all PII types.",
                    pii_type
                )
            }
            LimitError::FeatureNotAvailable { feature } => {
                write!(
                    f,
                    "Feature '{}' is not available in Lite version. Upgrade to Pro.",
                    feature
                )
            }
        }
    }
}
