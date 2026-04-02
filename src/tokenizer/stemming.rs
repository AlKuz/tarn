//! Language-aware stemming tokenizer.

use std::collections::HashMap;

use super::{Tokenizer, TokenizerConfig};
use crate::common::Configurable;
use lingua::{Language, LanguageDetector, LanguageDetectorBuilder};
use rust_stemmers::{Algorithm, Stemmer};

/// Language-aware stemming tokenizer.
///
/// Detects the language of input text and applies the appropriate stemmer.
/// New stemmers are created on demand and cached for reuse.
/// Falls back to English stemming when language detection fails.
pub struct StemmingTokenizer {
    stemmers: HashMap<Language, Stemmer>,
    detector: LanguageDetector,
}

impl std::fmt::Debug for StemmingTokenizer {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("StemmingTokenizer").finish()
    }
}

impl Default for StemmingTokenizer {
    fn default() -> Self {
        Self::new()
    }
}

impl StemmingTokenizer {
    pub fn new() -> Self {
        let languages = [
            Language::English,
            Language::French,
            Language::German,
            Language::Spanish,
            Language::Italian,
            Language::Portuguese,
            Language::Dutch,
            Language::Russian,
        ];
        let detector = LanguageDetectorBuilder::from_languages(&languages).build();
        let stemmers = languages
            .iter()
            .map(|&lang| (lang, Stemmer::create(Self::language_to_algorithm(lang))))
            .collect();
        Self { stemmers, detector }
    }

    fn language_to_algorithm(language: Language) -> Algorithm {
        match language {
            Language::English => Algorithm::English,
            Language::French => Algorithm::French,
            Language::German => Algorithm::German,
            Language::Spanish => Algorithm::Spanish,
            Language::Italian => Algorithm::Italian,
            Language::Portuguese => Algorithm::Portuguese,
            Language::Dutch => Algorithm::Dutch,
            Language::Russian => Algorithm::Russian,
        }
    }
}

impl Tokenizer for StemmingTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String> {
        let language = self
            .detector
            .detect_language_of(text)
            .unwrap_or(Language::English);

        let stemmer = self.stemmers.get(&language).unwrap();

        text.split_whitespace()
            .map(|s| {
                let cleaned: String = s
                    .chars()
                    .filter(|c| c.is_alphanumeric())
                    .collect::<String>()
                    .to_lowercase();
                if cleaned.is_empty() {
                    cleaned
                } else {
                    stemmer.stem(&cleaned).to_string()
                }
            })
            .filter(|s| !s.is_empty())
            .collect()
    }
}

impl Configurable for StemmingTokenizer {
    type Config = TokenizerConfig;

    fn config(&self) -> Self::Config {
        TokenizerConfig::Stemming
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stems_english_words() {
        let tokenizer = StemmingTokenizer::new();
        let tokens = tokenizer.tokenize("running quickly through the programming languages");
        // "running" -> "run", "quickly" -> "quick", "programming" -> "program"
        assert!(tokens.contains(&"run".to_string()));
        assert!(tokens.contains(&"quick".to_string()));
        assert!(tokens.contains(&"program".to_string()));
    }

    #[test]
    fn handles_empty_input() {
        let tokenizer = StemmingTokenizer::new();
        let tokens = tokenizer.tokenize("");
        assert!(tokens.is_empty());
    }

    #[test]
    fn removes_punctuation() {
        let tokenizer = StemmingTokenizer::new();
        let tokens = tokenizer.tokenize("Hello, world!");
        assert!(!tokens.is_empty());
        // All tokens should be lowercase with no punctuation
        for token in &tokens {
            assert!(token.chars().all(|c| c.is_alphanumeric()));
        }
    }

    #[test]
    fn get_config_returns_stemming() {
        let tokenizer = StemmingTokenizer::new();
        assert!(matches!(tokenizer.config(), TokenizerConfig::Stemming));
    }

    #[test]
    fn fallback_to_english() {
        let tokenizer = StemmingTokenizer::new();
        // Short text that may not be detectable — should fall back to English
        let tokens = tokenizer.tokenize("xyz");
        assert!(!tokens.is_empty());
    }

    #[test]
    fn dynamic_stemmer_creation() {
        let tokenizer = StemmingTokenizer::new();
        // German text should trigger German stemmer creation
        let tokens = tokenizer.tokenize(
            "Die Programmiersprachen sind sehr wichtig für die moderne Softwareentwicklung",
        );
        assert!(!tokens.is_empty());
    }
}
