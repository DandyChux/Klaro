use crate::pii_detector::{PiiMatch, PiiType};
use fake::Fake;
use fake::faker::address::en::*;
use fake::faker::internet::en::*;
use fake::faker::name::en::*;
use fake::faker::phone_number::en::*;
use rand::Rng;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum ScrubMethod {
    Remove,
    Hash,
    Fake,
    Mask,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScrubConfig {
    pub method: ScrubMethod,
    pub preserve_format: bool,
}

impl Default for ScrubConfig {
    fn default() -> Self {
        Self {
            method: ScrubMethod::Mask,
            preserve_format: true,
        }
    }
}

pub struct Scrubber {
    config: ScrubConfig,
}

impl Scrubber {
    pub fn new(config: ScrubConfig) -> Self {
        Self { config }
    }

    pub fn scrub_text(&self, text: &str, matches: &[PiiMatch]) -> String {
        if matches.is_empty() {
            return text.to_string();
        }

        let mut result = String::new();
        let mut last_end = 0;

        for m in matches {
            // Add text before this match
            if m.start > last_end {
                result.push_str(&text[last_end..m.start]);
            }

            // Add scrubbed value
            let replacement = self.get_replacement(&m.pii_type, &m.value);
            result.push_str(&replacement);

            last_end = m.end;
        }

        // Add remaining text
        if last_end < text.len() {
            result.push_str(&text[last_end..]);
        }

        result
    }

    fn get_replacement(&self, pii_type: &PiiType, original: &str) -> String {
        match self.config.method {
            ScrubMethod::Remove => self.get_removal_placeholder(pii_type),
            ScrubMethod::Hash => self.hash_value(original),
            ScrubMethod::Fake => self.generate_fake(pii_type, original),
            ScrubMethod::Mask => self.mask_value(pii_type, original),
        }
    }

    fn get_removal_placeholder(&self, pii_type: &PiiType) -> String {
        format!("[{} REMOVED]", pii_type.display_name().to_uppercase())
    }

    fn hash_value(&self, value: &str) -> String {
        let mut hasher = Sha256::new();
        hasher.update(value.as_bytes());
        let result = hasher.finalize();
        format!("#{}", &hex::encode(result)[..12])
    }

    fn mask_value(&self, pii_type: &PiiType, original: &str) -> String {
        match pii_type {
            PiiType::Email => {
                if let Some(at_pos) = original.find('@') {
                    let local = &original[..at_pos];
                    let domain = &original[at_pos..];
                    if local.len() > 2 {
                        format!("{}***{}", &local[..1], domain)
                    } else {
                        format!("***{}", domain)
                    }
                } else {
                    "***@***.***".to_string()
                }
            }
            PiiType::Phone => {
                let digits: String = original.chars().filter(|c| c.is_ascii_digit()).collect();
                if digits.len() >= 4 {
                    format!("***-***-{}", &digits[digits.len() - 4..])
                } else {
                    "***-***-****".to_string()
                }
            }
            PiiType::SSN => "***-**-****".to_string(),
            PiiType::CreditCard => {
                let digits: String = original.chars().filter(|c| c.is_ascii_digit()).collect();
                if digits.len() >= 4 {
                    format!("****-****-****-{}", &digits[digits.len() - 4..])
                } else {
                    "****-****-****-****".to_string()
                }
            }
            PiiType::IPAddress => "***.***.***.***".to_string(),
            PiiType::DateOfBirth => "**/**/****".to_string(),
            PiiType::Organization => fake::faker::company::en::CompanyName().fake(),
            _ => "*".repeat(original.len().min(20)),
        }
    }

    fn generate_fake(&self, pii_type: &PiiType, original: &str) -> String {
        let mut rng = rand::rng();

        match pii_type {
            PiiType::Email => SafeEmail().fake(),
            PiiType::Phone => PhoneNumber().fake(),
            PiiType::SSN => {
                format!(
                    "{:03}-{:02}-{:04}",
                    rng.random_range(100..899),
                    rng.random_range(1..99),
                    rng.random_range(1..9999)
                )
            }
            PiiType::CreditCard => {
                // Generate a fake but valid-looking card number
                format!(
                    "4{:03}-{:04}-{:04}-{:04}",
                    rng.random_range(0..999),
                    rng.random_range(0..9999),
                    rng.random_range(0..9999),
                    rng.random_range(0..9999)
                )
            }
            PiiType::IPAddress => {
                format!(
                    "{}.{}.{}.{}",
                    rng.random_range(1..255),
                    rng.random_range(0..255),
                    rng.random_range(0..255),
                    rng.random_range(1..255)
                )
            }
            PiiType::DateOfBirth => {
                format!(
                    "{:02}/{:02}/{:04}",
                    rng.random_range(1..12),
                    rng.random_range(1..28),
                    rng.random_range(1950..2005)
                )
            }
            PiiType::Address => SecondaryAddress().fake(),
            PiiType::Name => Name().fake(),
            PiiType::Passport => {
                format!("P{:08}", rng.random_range(10000000..99999999))
            }
            PiiType::DriversLicense => {
                format!("DL{:07}", rng.random_range(1000000..9999999))
            }
            PiiType::BankAccount => {
                format!("{:012}", rng.random_range(100000000000u64..999999999999u64))
            }
            PiiType::Organization => fake::faker::company::en::CompanyName().fake(),
        }
    }
}
