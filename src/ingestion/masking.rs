use regex::Regex;
use std::sync::OnceLock;

pub struct MaskingEngine;

impl MaskingEngine {
    pub fn clean(text: &str) -> String {
        static EMAIL: OnceLock<Regex> = OnceLock::new();
        static CREDENTIAL: OnceLock<Regex> = OnceLock::new();
        static JWT: OnceLock<Regex> = OnceLock::new();

        let email = EMAIL.get_or_init(|| {
            Regex::new(r"(?i)\b[a-z0-9._%+-]+@[a-z0-9.-]+\.[a-z]{2,}\b").unwrap()
        });
        let credential = CREDENTIAL.get_or_init(|| {
            Regex::new(
                r"\b(?:sk-[a-zA-Z0-9_-]{20,}|AKIA[0-9A-Z]{16}|gh[pousr]_[a-zA-Z0-9]{36}|xox[baprs]-[a-zA-Z0-9-]{20,})\b",
            )
            .unwrap()
        });
        let jwt = JWT.get_or_init(|| {
            Regex::new(r"\beyJ[A-Za-z0-9_-]+\.eyJ[A-Za-z0-9_-]+\.[A-Za-z0-9_-]+\b").unwrap()
        });

        let out = email.replace_all(text, "[REDACTED_EMAIL]");
        let out = credential.replace_all(&out, "[REDACTED_CREDENTIAL]");
        let out = jwt.replace_all(&out, "[REDACTED_CREDENTIAL]");
        out.into_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn redacts_email() {
        let out = MaskingEngine::clean("Contact developer@ucp.io for access.");
        assert!(out.contains("[REDACTED_EMAIL]"));
        assert!(!out.contains("developer@ucp.io"));
    }

    #[test]
    fn redacts_openai_key() {
        let out = MaskingEngine::clean("api: sk-liveSecretApiKey123456789abc end");
        assert!(out.contains("[REDACTED_CREDENTIAL]"));
        assert!(!out.contains("sk-liveSecretApiKey123456789abc"));
    }

    #[test]
    fn redacts_aws_access_key() {
        let out = MaskingEngine::clean("use AKIAIOSFODNN7EXAMPLE for access");
        assert!(out.contains("[REDACTED_CREDENTIAL]"));
        assert!(!out.contains("AKIAIOSFODNN7EXAMPLE"));
    }

    #[test]
    fn redacts_github_pat() {
        let pat = "ghp_1234567890abcdef1234567890abcdef1234"; // ghp_ + 36 chars
        let out = MaskingEngine::clean(&format!("token {pat} here"));
        assert!(out.contains("[REDACTED_CREDENTIAL]"));
        assert!(!out.contains(pat));
    }

    #[test]
    fn redacts_jwt() {
        let jwt = "eyJhbGciOiJIUzI1NiJ9.eyJzdWIiOiIxMjMifQ.signature_part_here";
        let out = MaskingEngine::clean(&format!("Authorization: Bearer {jwt}"));
        assert!(out.contains("[REDACTED_CREDENTIAL]"));
        assert!(!out.contains(jwt));
    }

    #[test]
    fn leaves_prose_untouched() {
        let raw = "The wizard stepped into the grand hall of Hogwarts. There were 142 staircases.";
        assert_eq!(MaskingEngine::clean(raw), raw);
    }

    #[test]
    fn leaves_numeric_strings_alone() {
        let raw = "Order 555-0199 contains 800 widgets and 415 sensors.";
        assert_eq!(MaskingEngine::clean(raw), raw);
    }

    #[test]
    fn handles_multiple_patterns() {
        let raw = "Email dev@x.io with sk-abcdefghijklmnopqrstuv and check.";
        let out = MaskingEngine::clean(raw);
        assert!(out.contains("[REDACTED_EMAIL]"));
        assert!(out.contains("[REDACTED_CREDENTIAL]"));
        assert!(!out.contains("dev@x.io"));
    }
}
