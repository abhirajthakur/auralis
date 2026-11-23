//! Refinement prompts: the engine's core instruction and built-in styles.
//!
//! Design rules baked into every prompt (product spec §9 / §4):
//! - never change the speaker's meaning
//! - never add information
//! - never summarize unless the style asks for it
//! - preserve names, numbers, URLs, emails, code identifiers
//! - return only the refined text

/// Core identity used by every style (custom included), per spec §9.
pub const ENGINE_IDENTITY: &str = "You are a professional speech-to-text editor. Convert spoken \
language into clean, natural, grammatically correct written text. Preserve the speaker's original \
meaning, intent, tone, names, and important details. Remove unnecessary filler words only when \
appropriate. Correct punctuation and grammar. Do not add information. Do not summarize unless \
requested. Return only the refined text.";

/// Guard rails appended to every style so providers behave predictably.
pub const OUTPUT_RULES: &str = "Rules:\n\
- Return ONLY the refined text. No preamble, quotes, markdown fences, or explanations.\n\
- Preserve the original language of the speech.\n\
- Keep technical terms, code identifiers, file paths, URLs, and email addresses exactly as spoken.\n\
- If the audio text is empty or unintelligible, return it unchanged.";

pub const SMART_FORMATTING: &str = "Apply smart formatting: add punctuation, capitalization, and \
paragraph breaks where natural. Keep the wording as spoken.";

pub const PROFESSIONAL: &str = "Rewrite in a professional register suitable for business writing, \
while preserving the speaker's meaning and level of detail.";

pub const CASUAL: &str = "Rewrite in a relaxed, friendly, conversational tone while preserving \
meaning and detail. Contractions are fine.";

pub const CONCISE: &str = "Make the text concise: remove filler and redundancy. Preserve every \
distinct fact, name, and number — condense phrasing only.";

pub const EMAIL: &str = "Format as an email body with an appropriate greeting and sign-off only if \
the speaker clearly intends an email. Keep content otherwise as spoken.";

pub const CHAT: &str = "Format as a chat message: short, natural, no formal greeting or sign-off \
unless spoken. Emoji only if the speaker asks for them.";

pub const TECHNICAL: &str = "Rewrite for developer audiences. Preserve code identifiers, \
commands, file paths, and API names verbatim. Use markdown code formatting for code fragments \
spoken as code (e.g. \"backtick function name backtick\").";

/// Compose the final system prompt: engine identity + style + output rules.
pub fn compose_system_prompt(style_prompt: &str) -> String {
    let style = style_prompt.trim();
    if style.is_empty() {
        format!("{ENGINE_IDENTITY}\n\n{OUTPUT_RULES}")
    } else {
        format!("{ENGINE_IDENTITY}\n\nStyle: {style}\n\n{OUTPUT_RULES}")
    }
}

/// Compose the user message for a refinement request.
pub fn compose_user_message(text: &str) -> String {
    format!("Refine the following transcript:\n\n{text}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_has_required_guarantees() {
        for needle in [
            "Preserve the speaker's original meaning",
            "Do not add information",
            "Do not summarize",
            "Return only the refined text",
        ] {
            assert!(ENGINE_IDENTITY.contains(needle), "missing: {needle}");
        }
    }

    #[test]
    fn compose_with_style_includes_both() {
        let p = compose_system_prompt(CONCISE);
        assert!(p.starts_with(ENGINE_IDENTITY));
        assert!(p.contains("Style:"));
        assert!(p.contains(CONCISE));
        assert!(p.ends_with(OUTPUT_RULES));
    }

    #[test]
    fn compose_empty_style_skips_style_line() {
        let p = compose_system_prompt("");
        assert!(!p.contains("Style:"));
        assert!(p.contains(ENGINE_IDENTITY));
    }

    #[test]
    fn user_message_wraps_transcript() {
        assert!(compose_user_message("hello world").contains("hello world"));
        assert!(compose_user_message("hi").starts_with("Refine"));
    }

    #[test]
    fn all_builtin_style_prompts_are_distinct() {
        let prompts = [
            SMART_FORMATTING,
            PROFESSIONAL,
            CASUAL,
            CONCISE,
            EMAIL,
            CHAT,
            TECHNICAL,
        ];
        let set: std::collections::HashSet<_> = prompts.iter().collect();
        assert_eq!(set.len(), prompts.len());
        assert!(prompts.iter().all(|p| !p.is_empty()));
    }
}
