// fuzz_targets/fuzz_engine.rs
// Fuzz the detection engine with arbitrary SourceFile input.

#![no_main]
use libfuzzer_sys::fuzz_target;
use cpd_core::{
    detect::detect,
    models::{Location, SourceFile, Token, TokenKind},
    store::MemoryStore,
};

fuzz_target!(|data: &[u8]| {
    if data.len() < 4 { return; }
    
    // Parse bytes into a small set of SourceFiles
    let num_files = (data[0] as usize % 3) + 1;
    let min_tokens = (data[1] as usize % 10) + 3;
    
    let mut files = Vec::new();
    let chunk_size = (data.len() - 2) / num_files;
    
    for file_idx in 0..num_files {
        let start = 2 + file_idx * chunk_size;
        let end = (start + chunk_size).min(data.len());
        if start >= end { continue; }
        
        let file_data = &data[start..end];
        let source_str = String::from_utf8_lossy(file_data);
        
        // Build tokens from the source words
        let tokens: Vec<Token> = source_str
            .split_whitespace()
            .take(200) // bound token count
            .enumerate()
            .map(|(i, word)| {
                let loc = Location { line: i as u32 + 1, column: 0, offset: i as u32 * 10 };
                Token {
                    kind: TokenKind::Other,
                    value: word.chars().take(20).collect(), // bound value length
                    format: "javascript".to_string(),
                    start: loc.clone(),
                    end: loc,
                }
            })
            .collect();
        
        files.push(SourceFile {
            id: format!("fuzz_file_{}.js", file_idx),
            format: "javascript".to_string(),
            tokens,
        });
    }
    
    // Must not panic
    let mut store = MemoryStore::new();
    let _ = detect(&files, min_tokens, &mut store);
});
