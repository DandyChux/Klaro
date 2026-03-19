use crate::ner::{NerEntityType, is_ner_available, predict_entities};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::{LazyLock, RwLock};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
pub enum PiiType {
    Email,
    Phone,
    SSN,
    CreditCard,
    IPAddress,
    DateOfBirth,
    Address,
    Name,
    Passport,
    DriversLicense,
    BankAccount,
    Organization,
}

impl PiiType {
    pub fn display_name(&self) -> &'static str {
        match self {
            PiiType::Email => "Email Address",
            PiiType::Phone => "Phone Number",
            PiiType::SSN => "Social Security Number",
            PiiType::CreditCard => "Credit Card Number",
            PiiType::IPAddress => "IP Address",
            PiiType::DateOfBirth => "Date of Birth",
            PiiType::Address => "Street Address",
            PiiType::Name => "Person Name",
            PiiType::Passport => "Passport Number",
            PiiType::DriversLicense => "Driver's License",
            PiiType::BankAccount => "Bank Account Number",
            PiiType::Organization => "Organization Name",
        }
    }

    pub fn all() -> Vec<PiiType> {
        vec![
            PiiType::Email,
            PiiType::Phone,
            PiiType::SSN,
            PiiType::CreditCard,
            PiiType::IPAddress,
            PiiType::DateOfBirth,
            PiiType::Address,
            PiiType::Name,
            PiiType::Passport,
            PiiType::DriversLicense,
            PiiType::BankAccount,
            PiiType::Organization,
        ]
    }

    pub fn uses_ner(&self) -> bool {
        matches!(
            self,
            PiiType::Name | PiiType::Address | PiiType::Organization
        )
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiiMatch {
    pub pii_type: PiiType,
    pub value: String,
    pub start: usize,
    pub end: usize,
}

// ============================================================================
// Regex Patterns
// ============================================================================

static EMAIL_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)[a-zA-Z0-9._%+-]+@[a-zA-Z0-9.-]+\.[a-zA-Z]{2,}").unwrap());

static PHONE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:\+?1[-.\s]?)?\(?[0-9]{3}\)?[-.\s]?[0-9]{3}[-.\s]?[0-9]{4}").unwrap()
});

static SSN_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\d{3}[-\s]?\d{2}[-\s]?\d{4}\b").unwrap());

static BANK_ACCOUNT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b(?:\d{4}[-\s]\d{4}[-\s]\d{4}[-\s]\d{4}|\d{16})\b").unwrap());

static CREDIT_CARD_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:4[0-9]{12}(?:[0-9]{3})?|5[1-5][0-9]{14}|3[47][0-9]{13}|6(?:011|5[0-9]{2})[0-9]{12}|(?:2131|1800|35\d{3})\d{11})\b").unwrap()
});

static IP_ADDRESS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b").unwrap()
});

static DATE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:0?[1-9]|1[0-2])[/\-](?:0?[1-9]|[12][0-9]|3[01])[/\-](?:19|20)?\d{2}\b|\b(?:19|20)\d{2}[/\-](?:0?[1-9]|1[0-2])[/\-](?:0?[1-9]|[12][0-9]|3[01])\b").unwrap()
});

static PASSPORT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[A-Z]{1,2}[0-9]{6,9}\b").unwrap());

static DRIVERS_LICENSE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[A-Z]{1,2}[0-9]{5,8}\b").unwrap());

// ============================================================================
// PII Detector
// ============================================================================

pub const NER_BATCH_SIZE: usize = 128;

pub struct PiiDetector {
    enabled_types: Vec<PiiType>,
    /// Cache for NER results - avoids re-running the model for duplicate text
    ner_cache: RwLock<HashMap<String, Vec<PiiMatch>>>,
}

impl PiiDetector {
    pub fn new(enabled_types: Vec<PiiType>) -> Self {
        Self {
            enabled_types,
            ner_cache: RwLock::new(HashMap::new()),
        }
    }

    pub fn detect(&self, texts: &[&str]) -> Vec<PiiMatch> {
        let mut matches = Vec::new();

        // Process regex-based PII detection for each text
        for (text_idx, text) in texts.iter().enumerate() {
            for pii_type in &self.enabled_types {
                if !pii_type.uses_ner() {
                    if let Some(regex) = self.get_regex(pii_type) {
                        for mat in regex.find_iter(text) {
                            // Validate the match based on type
                            if self.validate_match(pii_type, mat.as_str()) {
                                matches.push(PiiMatch {
                                    pii_type: pii_type.clone(),
                                    value: mat.as_str().to_string(),
                                    start: mat.start(),
                                    end: mat.end(),
                                });
                            }
                        }
                    }
                }
            }
        }

        if self.uses_ner() && is_ner_available() {
            // Filter texts that are long enough for NER
            let valid_texts: Vec<(usize, &str)> = texts
                .iter()
                .enumerate()
                .filter(|(_, t)| t.len() >= 3)
                .map(|(i, t)| (i, *t))
                .collect();

            if !valid_texts.is_empty() {
                // Check cache for each text
                let mut uncached_indices: Vec<usize> = Vec::new();
                let mut uncached_texts: Vec<&str> = Vec::new();
                let mut cached_matches: Vec<PiiMatch> = Vec::new();

                {
                    let cache = self
                        .ner_cache
                        .read()
                        .unwrap_or_else(|poisoned| poisoned.into_inner());

                    for (_, text) in &valid_texts {
                        if let Some(cached) = cache.get(*text) {
                            cached_matches.extend(cached.clone());
                        } else {
                            uncached_indices.push(uncached_texts.len());
                            uncached_texts.push(text);
                        }
                    }
                }

                // Process uncached texts through NER
                if !uncached_texts.is_empty() {
                    if let Ok(batch_entities) = predict_entities(&uncached_texts) {
                        let mut cache = self
                            .ner_cache
                            .write()
                            .unwrap_or_else(|poisoned| poisoned.into_inner());

                        for (text_idx, entities) in batch_entities.into_iter().enumerate() {
                            let text = uncached_texts[text_idx];
                            let mut ner_matches = Vec::new();

                            for entity in entities {
                                // Map NER entity types to PII types
                                let pii_type = match entity.entity_type {
                                    NerEntityType::Person
                                        if self.enabled_types.contains(&PiiType::Name) =>
                                    {
                                        Some(PiiType::Name)
                                    }
                                    NerEntityType::Location
                                        if self.enabled_types.contains(&PiiType::Address) =>
                                    {
                                        Some(PiiType::Address)
                                    }
                                    NerEntityType::Organization
                                        if self.enabled_types.contains(&PiiType::Organization) =>
                                    {
                                        Some(PiiType::Organization)
                                    }
                                    _ => None,
                                };

                                if let Some(pii_type) = pii_type {
                                    // Only include confident predictions
                                    if entity.confidence >= 0.70 {
                                        ner_matches.push(PiiMatch {
                                            pii_type,
                                            value: entity.text,
                                            start: entity.start,
                                            end: entity.end,
                                        });
                                    }
                                }
                            }

                            if cache.len() < 50_000 {
                                cache.insert(text.to_string(), ner_matches.clone());
                            }
                            matches.extend(ner_matches);
                        }
                    }
                }

                matches.extend(cached_matches);
            }
        }

        // Sort by start position and remove overlapping matches
        matches.sort_by_key(|m| m.start);
        self.remove_overlapping(matches)
    }

    fn get_regex(&self, pii_type: &PiiType) -> Option<&Regex> {
        match pii_type {
            PiiType::Email => Some(&EMAIL_REGEX),
            PiiType::Phone => Some(&PHONE_REGEX),
            PiiType::SSN => Some(&SSN_REGEX),
            PiiType::CreditCard => Some(&CREDIT_CARD_REGEX),
            PiiType::IPAddress => Some(&IP_ADDRESS_REGEX),
            PiiType::DateOfBirth => Some(&DATE_REGEX),
            PiiType::Passport => Some(&PASSPORT_REGEX),
            PiiType::DriversLicense => Some(&DRIVERS_LICENSE_REGEX),
            PiiType::BankAccount => Some(&BANK_ACCOUNT_REGEX),
            PiiType::Name | PiiType::Address | PiiType::Organization => None,
        }
    }

    /// Whether any enabled PII type requires the NER model
    pub fn uses_ner(&self) -> bool {
        self.enabled_types.iter().any(|t| t.uses_ner())
    }

    /// Regex-only detection (no NER). Very fast.
    pub fn detect_regex(&self, text: &str) -> Vec<PiiMatch> {
        let mut matches = Vec::new();

        for pii_type in &self.enabled_types {
            if pii_type.uses_ner() {
                continue; // skip NER types entirely
            }
            if let Some(regex) = self.get_regex(pii_type) {
                for mat in regex.find_iter(text) {
                    if self.validate_match(pii_type, mat.as_str()) {
                        matches.push(PiiMatch {
                            pii_type: pii_type.clone(),
                            value: mat.as_str().to_string(),
                            start: mat.start(),
                            end: mat.end(),
                        });
                    }
                }
            }
        }

        matches.sort_by_key(|m| m.start);
        self.remove_overlapping(matches)
    }

    /// Batch NER detection for multiple texts.
    /// Checks cache for each text, batches uncached texts through the model
    /// in sub-batches of NER_BATCH_SIZE, cache results, returns all
    pub fn detect_ner(&self, texts: &[&str]) -> Vec<Vec<PiiMatch>> {
        if !self.uses_ner() {
            return vec![Vec::new(); texts.len()];
        }

        let mut results: Vec<Option<Vec<PiiMatch>>> = vec![None; texts.len()];
        let mut uncached_indices: Vec<usize> = Vec::new();
        let mut uncached_texts: Vec<&str> = Vec::new();

        // Check cache for each text
        {
            let cache = self.ner_cache.read().unwrap_or_else(|p| p.into_inner());

            for (i, text) in texts.iter().enumerate() {
                if text.len() < 3 {
                    results[i] = Some(Vec::new());
                } else if let Some(cached) = cache.get(*text) {
                    results[i] = Some(cached.clone());
                } else {
                    uncached_indices.push(i);
                    uncached_texts.push(text);
                }
            }
        }

        // Process uncached texts in sub-batches through the model
        if !uncached_texts.is_empty() {
            for sub_batch_start in (0..uncached_texts.len()).step_by(NER_BATCH_SIZE) {
                let sub_batch_end = (sub_batch_start + NER_BATCH_SIZE).min(uncached_texts.len());
                let sub_batch = &uncached_texts[sub_batch_start..sub_batch_end];

                match predict_entities(sub_batch) {
                    Ok(batch_entities) => {
                        let mut cache = self.ner_cache.write().unwrap_or_else(|p| p.into_inner());

                        for (local_idx, entities) in batch_entities.into_iter().enumerate() {
                            let global_idx = uncached_indices[sub_batch_start + local_idx];
                            let text = uncached_texts[sub_batch_start + local_idx];

                            let ner_matches: Vec<PiiMatch> = entities
                                .into_iter()
                                .filter_map(|entity| {
                                    let pii_type = match entity.entity_type {
                                        NerEntityType::Person
                                            if self.enabled_types.contains(&PiiType::Name) =>
                                        {
                                            Some(PiiType::Name)
                                        }
                                        NerEntityType::Location
                                            if self.enabled_types.contains(&PiiType::Address) =>
                                        {
                                            Some(PiiType::Address)
                                        }
                                        NerEntityType::Organization
                                            if self
                                                .enabled_types
                                                .contains(&PiiType::Organization) =>
                                        {
                                            Some(PiiType::Organization)
                                        }
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
                                .collect();

                            if cache.len() < 50_000 {
                                cache.insert(text.to_string(), ner_matches.clone());
                            }
                            results[global_idx] = Some(ner_matches);
                        }
                    }
                    Err(_) => {
                        for i in sub_batch_start..sub_batch_end {
                            results[uncached_indices[i]] = Some(Vec::new());
                        }
                    }
                }
            }
        }

        results.into_iter().map(|r| r.unwrap_or_default()).collect()
    }

    /// Public static version of overlap removal (used by process_tabular
    /// after merging regex + NER matches from different sources)
    pub fn dedup_overlapping(matches: Vec<PiiMatch>) -> Vec<PiiMatch> {
        let mut result = Vec::new();
        let mut last_end = 0;

        for m in matches {
            if m.start >= last_end {
                last_end = m.end;
                result.push(m);
            }
        }

        result
    }

    fn validate_match(&self, pii_type: &PiiType, value: &str) -> bool {
        match pii_type {
            PiiType::CreditCard => self.validate_credit_card(value),
            PiiType::SSN => self.validate_ssn(value),
            _ => true,
        }
    }

    fn validate_credit_card(&self, value: &str) -> bool {
        // Luhn algorithm validation
        let digits: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() < 13 || digits.len() > 19 {
            return false;
        }

        let mut sum = 0;
        let mut alternate = false;

        for c in digits.chars().rev() {
            let mut n = c.to_digit(10).unwrap();
            if alternate {
                n *= 2;
                if n > 9 {
                    n -= 9;
                }
            }
            sum += n;
            alternate = !alternate;
        }

        sum % 10 == 0
    }

    fn validate_ssn(&self, value: &str) -> bool {
        let digits: String = value.chars().filter(|c| c.is_ascii_digit()).collect();
        if digits.len() != 9 {
            return false;
        }

        // SSN cannot start with 000, 666, or 900-999
        let area: u32 = digits[0..3].parse().unwrap_or(0);
        if area == 0 || area == 666 || area >= 900 {
            return false;
        }

        // Group number cannot be 00
        let group: u32 = digits[3..5].parse().unwrap_or(0);
        if group == 0 {
            return false;
        }

        // Serial number cannot be 0000
        let serial: u32 = digits[5..9].parse().unwrap_or(0);
        serial != 0
    }

    fn remove_overlapping(&self, matches: Vec<PiiMatch>) -> Vec<PiiMatch> {
        let mut result = Vec::new();
        let mut last_end = 0;

        for m in matches {
            if m.start >= last_end {
                last_end = m.end;
                result.push(m);
            }
        }

        result
    }
}
