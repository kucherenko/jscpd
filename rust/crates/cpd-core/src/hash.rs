// hash.rs
// Attribution: inspired by jscpd-rs rolling hash approach; rewritten independently.

use xxhash_rust::xxh3::xxh3_64;

/// Fibonacci hashing constant for good bit distribution.
pub const HASH_BASE: u64 = 0x9e3779b97f4a7c15;

/// Compute a deterministic hash for a single token given its kind byte and value string.
/// Uses xxh3_64 XORed with kind cast to u64.
pub fn token_hash(kind: u8, value: &str) -> u64 {
    xxh3_64(value.as_bytes()) ^ (kind as u64)
}

/// Compute the initial polynomial hash of a window of token hashes.
/// hash = h[0]*BASE^(n-1) + h[1]*BASE^(n-2) + ... + h[n-1]*BASE^0
/// Uses wrapping arithmetic throughout.
pub fn hash_window(hashes: &[u64]) -> u64 {
    hashes.iter().fold(0u64, |acc, &h| acc.wrapping_mul(HASH_BASE).wrapping_add(h))
}

/// Roll the hash one position: remove `outgoing` (the token leaving the window),
/// add `incoming` (the token entering the window).
///
/// `window_power` must be precomputed by the caller as `base_pow(window_size - 1)`
/// **once per format group** before the sliding-window loop — not on every call.
/// This eliminates an O(window_size) loop from the hot path.
///
/// If per-language min_tokens is introduced in future, recompute `window_power`
/// per `detect_in_group` invocation using that group's min_tokens value.
///
/// new_hash = (current - outgoing * window_power) * BASE + incoming
/// All arithmetic is wrapping.
pub fn roll(current: u64, outgoing: u64, incoming: u64, window_power: u64) -> u64 {
    current
        .wrapping_sub(outgoing.wrapping_mul(window_power))
        .wrapping_mul(HASH_BASE)
        .wrapping_add(incoming)
}

/// Compute HASH_BASE^n using wrapping multiplication.
/// Call once per format group to obtain the `window_power` argument for `roll()`.
pub fn base_pow(n: usize) -> u64 {
    let mut result = 1u64;
    for _ in 0..n {
        result = result.wrapping_mul(HASH_BASE);
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_hash_is_deterministic() {
        let h1 = token_hash(1, "function");
        let h2 = token_hash(1, "function");
        assert_eq!(h1, h2);
    }

    #[test]
    fn token_hash_differs_by_kind() {
        let h1 = token_hash(1, "x");
        let h2 = token_hash(2, "x");
        assert_ne!(h1, h2);
    }

    #[test]
    fn hash_window_single_element() {
        let h = token_hash(0, "a");
        // Window of 1: fold with initial 0 → 0 * BASE + h = h
        assert_eq!(hash_window(&[h]), h);
    }

    #[test]
    fn roll_matches_naive_recompute() {
        let a = token_hash(0, "a");
        let b = token_hash(0, "b");
        let c = token_hash(0, "c");
        let d = token_hash(0, "d");

        let initial = hash_window(&[a, b, c]);
        let wp = base_pow(3 - 1);
        let rolled = roll(initial, a, d, wp);
        let naive = hash_window(&[b, c, d]);
        assert_eq!(rolled, naive, "rolled hash must match naive recomputation");
    }

    #[test]
    fn roll_window_of_one() {
        let a = token_hash(0, "hello");
        let b = token_hash(0, "world");
        let initial = hash_window(&[a]);
        let wp = base_pow(1 - 1); // BASE^0 = 1
        let rolled = roll(initial, a, b, wp);
        let naive = hash_window(&[b]);
        assert_eq!(rolled, naive);
    }

    #[test]
    fn hash_window_empty_is_zero() {
        assert_eq!(hash_window(&[]), 0u64);
    }
}
