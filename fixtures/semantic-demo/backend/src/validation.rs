//! Request validation for the account endpoints.

use crate::error::FieldError;

const MAX_EMAIL_LEN: usize = 254;
const MAX_LOCAL_PART_LEN: usize = 64;
const COMMON_PASSWORDS: [&str; 6] = [
    "password", "123456", "qwerty", "letmein", "iloveyou", "admin",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Strength {
    Weak,
    Fair,
    Strong,
}

/// Accepts `local@domain.tld` addresses whose domain has at least two labels.
pub fn validate_email(input: &str) -> Result<(), FieldError> {
    let email = input.trim();
    if email.is_empty() {
        return Err(FieldError::new("email", "Email is required"));
    }
    if email.len() > MAX_EMAIL_LEN {
        return Err(FieldError::new("email", "Email is too long"));
    }
    let Some((local, domain)) = email.split_once('@') else {
        return Err(FieldError::new("email", "Email must contain @"));
    };
    if local.is_empty() || local.len() > MAX_LOCAL_PART_LEN || domain.contains('@') {
        return Err(FieldError::new("email", "Email address is invalid"));
    }
    let labels: Vec<&str> = domain.split('.').collect();
    let domain_ok = labels.len() >= 2
        && labels.iter().all(|label| {
            !label.is_empty()
                && !label.starts_with('-')
                && !label.ends_with('-')
                && label.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
        });
    if !domain_ok {
        return Err(FieldError::new("email", "Email domain is invalid"));
    }
    Ok(())
}

/// Scores a password by length and character variety.
pub fn password_strength(password: &str) -> Strength {
    if COMMON_PASSWORDS.contains(&password.to_lowercase().as_str()) {
        return Strength::Weak;
    }
    let mut score = 0;
    if password.chars().count() >= 8 {
        score += 1;
    }
    if password.chars().count() >= 12 {
        score += 1;
    }
    if password.chars().any(|c| c.is_lowercase()) && password.chars().any(|c| c.is_uppercase()) {
        score += 1;
    }
    if password.chars().any(|c| c.is_ascii_digit()) {
        score += 1;
    }
    if password.chars().any(|c| !c.is_alphanumeric()) {
        score += 1;
    }
    match score {
        0..=2 => Strength::Weak,
        3 => Strength::Fair,
        _ => Strength::Strong,
    }
}

/// Product codes look like `ABC-1234`: three letters, a dash, four digits.
pub fn validate_sku(input: &str) -> Result<String, FieldError> {
    let sku = input.trim().to_ascii_uppercase();
    let mut parts = sku.splitn(2, '-');
    let prefix = parts.next().unwrap_or_default();
    let number = parts.next().unwrap_or_default();
    let prefix_ok = prefix.len() == 3 && prefix.chars().all(|c| c.is_ascii_uppercase());
    let number_ok = number.len() == 4 && number.chars().all(|c| c.is_ascii_digit());
    if !prefix_ok || !number_ok {
        return Err(FieldError::new("sku", "SKU must look like ABC-1234"));
    }
    if number == "0000" {
        return Err(FieldError::new("sku", "SKU number cannot be zero"));
    }
    Ok(sku)
}
