//! Card checks done before the payment provider is called.

/// Luhn checksum over the digits of a card number; spaces and dashes are
/// allowed as separators.
pub fn is_valid_card_number(input: &str) -> bool {
    let digits: Vec<u32> = input
        .chars()
        .filter(|c| !matches!(c, ' ' | '-'))
        .map(|c| c.to_digit(10))
        .collect::<Option<Vec<u32>>>()
        .unwrap_or_default();
    if !(12..=19).contains(&digits.len()) {
        return false;
    }
    let sum: u32 = digits
        .iter()
        .rev()
        .enumerate()
        .map(|(i, &d)| {
            if i % 2 == 1 {
                let doubled = d * 2;
                if doubled > 9 { doubled - 9 } else { doubled }
            } else {
                d
            }
        })
        .sum();
    sum % 10 == 0
}
