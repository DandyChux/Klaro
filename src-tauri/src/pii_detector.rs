use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

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
        ]
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PiiMatch {
    pub pii_type: PiiType,
    pub value: String,
    pub start: usize,
    pub end: usize,
}

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

static CREDIT_CARD_SPACED_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b\d{4}[-\s]\d{4}[-\s]\d{4}[-\s]\d{4}\b").unwrap());

static IP_ADDRESS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.){3}(?:25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\b").unwrap()
});

static DATE_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:0?[1-9]|1[0-2])[/\-](?:0?[1-9]|[12][0-9]|3[01])[/\-](?:19|20)?\d{2}\b|\b(?:19|20)\d{2}[/\-](?:0?[1-9]|1[0-2])[/\-](?:0?[1-9]|[12][0-9]|3[01])\b").unwrap()
});

static ADDRESS_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\d{1,5}\s+[\w\s]+(?:street|st|avenue|ave|road|rd|boulevard|blvd|drive|dr|lane|ln|court|ct|way|place|pl|circle|cir)\.?(?:\s+(?:apt|apartment|suite|ste|unit|#)\s*\d+[a-z]?)?").unwrap()
});

static NAME_REGEX: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\b(?:Mr\.?|Mrs\.?|Ms\.?|Dr\.?|Prof\.?)\s+[A-Z][a-z]+(?:\s+[A-Z][a-z]+)+").unwrap()
});

static PASSPORT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[A-Z]{1,2}[0-9]{6,9}\b").unwrap());

static DRIVERS_LICENSE_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[A-Z]{1,2}[0-9]{5,8}\b").unwrap());

static BANK_ACCOUNT_REGEX: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"\b[0-9]{8,17}\b").unwrap());

pub struct PiiDetector {
    enabled_types: Vec<PiiType>,
}

impl PiiDetector {
    pub fn new(enabled_types: Vec<PiiType>) -> Self {
        Self { enabled_types }
    }

    pub fn detect(&self, text: &str) -> Vec<PiiMatch> {
        let mut matches = Vec::new();

        for pii_type in &self.enabled_types {
            let regex = self.get_regex(pii_type);
            if let Some(regex) = regex {
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
            PiiType::Address => Some(&ADDRESS_REGEX),
            PiiType::Name => Some(&NAME_REGEX),
            PiiType::Passport => Some(&PASSPORT_REGEX),
            PiiType::DriversLicense => Some(&DRIVERS_LICENSE_REGEX),
            PiiType::BankAccount => Some(&BANK_ACCOUNT_REGEX),
        }
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
