use cpd_core::models::Token;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Mode {
    #[default]
    Mild,
    Weak,
    Strict,
}

impl Mode {
    pub fn from_str(s: &str) -> Self {
        match s {
            "weak" => Self::Weak,
            "strict" => Self::Strict,
            _ => Self::Mild,
        }
    }
}

/// Tokenize source code in the given format with the given mode.
/// Returns a Vec<Token>. Never panics on empty input — returns empty Vec.
pub fn tokenize(format: &str, source: &str, mode: Mode) -> Vec<Token> {
    let raw = dispatch_tokenizer(format, source, mode);
    crate::filter::apply_mode(raw, mode)
}

fn dispatch_tokenizer(format: &str, source: &str, mode: Mode) -> Vec<Token> {
    match format {
        "javascript" | "typescript" | "jsx" | "tsx" => {
            crate::javascript::tokenize_js(source, format)
        }
        "vue" | "svelte" | "astro" => {
            crate::sfc::tokenize_sfc(source, format, mode)
        }
        "markdown" | "md" => {
            crate::markdown::tokenize_markdown(source, mode)
        }
        _ => crate::generic::tokenize_generic(source, format),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mode_from_str_defaults_to_mild() {
        assert_eq!(Mode::from_str("unknown"), Mode::Mild);
        assert_eq!(Mode::from_str("mild"), Mode::Mild);
    }

    #[test]
    fn mode_from_str_weak() {
        assert_eq!(Mode::from_str("weak"), Mode::Weak);
    }

    #[test]
    fn mode_from_str_strict() {
        assert_eq!(Mode::from_str("strict"), Mode::Strict);
    }
}
