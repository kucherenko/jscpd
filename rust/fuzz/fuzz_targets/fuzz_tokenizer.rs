// fuzz_targets/fuzz_tokenizer.rs
// Fuzz the tokenizer with arbitrary bytes across multiple formats.

#![no_main]
use libfuzzer_sys::fuzz_target;
use cpd_tokenizer::tokenizer::{tokenize, Mode};

static FORMATS: &[&str] = &[
    "javascript", "typescript", "python", "go", "java",
    "rust", "c", "cpp", "ruby", "php",
];

fuzz_target!(|data: &[u8]| {
    // Use first byte to select format, rest as source
    if data.is_empty() { return; }
    let format_idx = data[0] as usize % FORMATS.len();
    let format = FORMATS[format_idx];
    let source = String::from_utf8_lossy(&data[1..]);
    
    // Must not panic on any input
    let _ = tokenize(format, &source, Mode::Mild);
});
