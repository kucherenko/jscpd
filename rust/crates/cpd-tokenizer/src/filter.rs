// filter.rs — Token filtering by detection mode.
// Part of cpd-tokenizer. See crate root for attribution.

use cpd_core::models::{Token, TokenKind};
use crate::tokenizer::Mode;

/// Apply the detection mode filter to a token stream.
/// All Ignore tokens are removed in every mode.
///
/// Mild:   remove Whitespace. Keep all others including comments.
/// Weak:   additionally remove Comment, BlockComment.
/// Strict: keep everything except Ignore (whitespace preserved).
pub fn apply_mode(tokens: Vec<Token>, mode: Mode) -> Vec<Token> {
    tokens.into_iter().filter(|t| keep_token(t, mode)).collect()
}

fn keep_token(token: &Token, mode: Mode) -> bool {
    if token.kind == TokenKind::Ignore {
        return false;
    }
    match mode {
        Mode::Mild => !matches!(token.kind, TokenKind::Whitespace),
        Mode::Weak => !matches!(
            token.kind,
            TokenKind::Whitespace | TokenKind::Comment | TokenKind::BlockComment
        ),
        Mode::Strict => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cpd_core::models::{Location, Token, TokenKind};
    use crate::tokenizer::Mode;

    fn make_token(kind: TokenKind, value: &str) -> Token {
        let loc = Location { line: 1, column: 0, offset: 0 };
        Token {
            kind,
            value: value.to_string(),
            format: "test".to_string(),
            start: loc.clone(),
            end: loc,
        }
    }

    #[test]
    fn mild_removes_whitespace() {
        let tokens = vec![
            make_token(TokenKind::Keyword, "function"),
            make_token(TokenKind::Whitespace, " "),
            make_token(TokenKind::Other, "foo"),
        ];
        let filtered = apply_mode(tokens, Mode::Mild);
        assert_eq!(filtered.len(), 2);
        assert!(filtered.iter().all(|t| t.kind != TokenKind::Whitespace));
    }

    #[test]
    fn mild_keeps_comments() {
        let tokens = vec![
            make_token(TokenKind::Comment, "// hello"),
            make_token(TokenKind::Keyword, "function"),
        ];
        let filtered = apply_mode(tokens, Mode::Mild);
        assert_eq!(filtered.len(), 2, "Mild mode must keep comments");
    }

    #[test]
    fn weak_removes_comments_and_whitespace() {
        let tokens = vec![
            make_token(TokenKind::Comment, "// hello"),
            make_token(TokenKind::BlockComment, "/* big */"),
            make_token(TokenKind::Whitespace, " "),
            make_token(TokenKind::Keyword, "return"),
        ];
        let filtered = apply_mode(tokens, Mode::Weak);
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].kind, TokenKind::Keyword);
    }

    #[test]
    fn strict_keeps_whitespace() {
        let tokens = vec![
            make_token(TokenKind::Keyword, "function"),
            make_token(TokenKind::Whitespace, " "),
            make_token(TokenKind::Other, "foo"),
        ];
        let filtered = apply_mode(tokens, Mode::Strict);
        assert_eq!(filtered.len(), 3, "Strict mode keeps whitespace");
    }

    #[test]
    fn all_modes_remove_ignore_tokens() {
        for mode in [Mode::Mild, Mode::Weak, Mode::Strict] {
            let tokens = vec![
                make_token(TokenKind::Ignore, "secret"),
                make_token(TokenKind::Keyword, "function"),
            ];
            let filtered = apply_mode(tokens, mode);
            assert!(
                filtered.iter().all(|t| t.kind != TokenKind::Ignore),
                "Mode {:?} must remove Ignore tokens",
                mode
            );
        }
    }

    #[test]
    fn empty_input_returns_empty() {
        let filtered = apply_mode(vec![], Mode::Mild);
        assert!(filtered.is_empty());
    }
}
