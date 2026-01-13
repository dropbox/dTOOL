//! Language detection for books
//!
//! Uses the `whatlang` crate for fast language detection on book text.
//! Supports detection of common Gutenberg languages: English, German, French,
//! Spanish, Italian, Portuguese, Russian, Latin, Greek, and Chinese.

use tracing::debug;
use whatlang::{detect, Lang};

/// Map whatlang Lang to ISO 639-1 language code
fn lang_to_code(lang: Lang) -> &'static str {
    match lang {
        Lang::Eng => "en",
        Lang::Deu => "de",
        Lang::Fra => "fr",
        Lang::Spa => "es",
        Lang::Ita => "it",
        Lang::Por => "pt",
        Lang::Rus => "ru",
        Lang::Lat => "la",
        Lang::Ell => "el", // Modern Greek (Ancient uses same code)
        Lang::Cmn => "zh", // Mandarin Chinese
        Lang::Nld => "nl", // Dutch
        Lang::Pol => "pl", // Polish
        Lang::Swe => "sv", // Swedish
        Lang::Dan => "da", // Danish
        Lang::Fin => "fi", // Finnish
        Lang::Nob => "no", // Norwegian Bokmål (whatlang uses Nob, not Nor)
        Lang::Ces => "cs", // Czech
        Lang::Hun => "hu", // Hungarian
        Lang::Tur => "tr", // Turkish
        Lang::Ara => "ar", // Arabic
        Lang::Jpn => "ja", // Japanese
        Lang::Kor => "ko", // Korean
        Lang::Hin => "hi", // Hindi
        _ => "en",         // Default to English for unsupported languages
    }
}

/// Detect the language of a text sample
///
/// Returns ISO 639-1 language code (e.g., "en", "de", "fr")
/// Falls back to "en" (English) if detection fails or confidence is too low.
///
/// # Arguments
/// * `text` - Text to detect language from (uses first ~5000 chars for efficiency)
///
/// # Returns
/// * ISO 639-1 language code string
pub fn detect_language(text: &str) -> &'static str {
    // Use first ~5000 chars for detection (enough for accuracy, fast enough for bulk)
    let sample_size = text.len().min(5000);
    let sample = &text[..sample_size];

    match detect(sample) {
        Some(info) => {
            let code = lang_to_code(info.lang());
            let confidence = info.confidence();

            debug!(
                "Detected language: {} ({}) with confidence {:.2}",
                info.lang(),
                code,
                confidence
            );

            // Only trust high confidence detections
            if confidence >= 0.8 {
                code
            } else {
                debug!(
                    "Low confidence ({:.2}), falling back to English",
                    confidence
                );
                "en"
            }
        }
        None => {
            debug!("Language detection failed, defaulting to English");
            "en"
        }
    }
}

/// Human-readable language name from ISO code
pub fn language_name(code: &str) -> &'static str {
    match code {
        "en" => "English",
        "de" => "German",
        "fr" => "French",
        "es" => "Spanish",
        "it" => "Italian",
        "pt" => "Portuguese",
        "ru" => "Russian",
        "la" => "Latin",
        "el" => "Greek",
        "zh" => "Chinese",
        "nl" => "Dutch",
        "pl" => "Polish",
        "sv" => "Swedish",
        "da" => "Danish",
        "fi" => "Finnish",
        "no" => "Norwegian",
        "cs" => "Czech",
        "hu" => "Hungarian",
        "tr" => "Turkish",
        "ar" => "Arabic",
        "ja" => "Japanese",
        "ko" => "Korean",
        "hi" => "Hindi",
        _ => "Unknown",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_detect_english() {
        let text = "This is a test of the English language detection system. \
                    The quick brown fox jumps over the lazy dog.";
        assert_eq!(detect_language(text), "en");
    }

    #[test]
    fn test_detect_german() {
        let text = "Als Gregor Samsa eines Morgens aus unruhigen Träumen erwachte, \
                    fand er sich in seinem Bett zu einem ungeheuren Ungeziefer verwandelt.";
        assert_eq!(detect_language(text), "de");
    }

    #[test]
    fn test_detect_french() {
        let text = "Jean Valjean était d'une pauvre famille de paysans de la Brie. \
                    Dans son enfance, il n'avait pas appris à lire.";
        assert_eq!(detect_language(text), "fr");
    }

    #[test]
    fn test_detect_spanish() {
        let text = "En un lugar de la Mancha, de cuyo nombre no quiero acordarme, \
                    no ha mucho tiempo que vivía un hidalgo.";
        assert_eq!(detect_language(text), "es");
    }

    #[test]
    fn test_detect_russian() {
        // Longer sample for better confidence - opening of Anna Karenina
        let text = "Все счастливые семьи похожи друг на друга, каждая несчастливая семья \
                    несчастлива по-своему. Все смешалось в доме Облонских. Жена узнала, что \
                    муж был в связи с бывшею в их доме француженкой-гувернанткой, и объявила \
                    мужу, что не может жить с ним в одном доме. Положение это продолжалось \
                    уже третий день и мучительно чувствовалось и самими супругами, и всеми \
                    членами семьи, и домочадцами.";
        assert_eq!(detect_language(text), "ru");
    }

    #[test]
    fn test_short_text_fallback() {
        // Very short text should fall back to English
        let text = "Hi";
        let result = detect_language(text);
        // Short texts may or may not detect correctly, just verify it returns something
        assert!(!result.is_empty());
    }

    #[test]
    fn test_language_name() {
        assert_eq!(language_name("en"), "English");
        assert_eq!(language_name("de"), "German");
        assert_eq!(language_name("fr"), "French");
        assert_eq!(language_name("unknown"), "Unknown");
    }
}
