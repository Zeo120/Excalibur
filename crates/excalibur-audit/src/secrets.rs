use regex::Regex;

pub struct SecretFinding {
    pub rule: String,
    pub matched_text: String,
}

pub struct SecretScanner {
    patterns: Vec<(&'static str, Regex)>,
}

impl SecretScanner {
    pub fn new() -> Self {
        let patterns = vec![
            ("Generic API Key", Regex::new(r#"(?i)(?:api_key|apikey|secret)[ =:"']{1,4}([a-zA-Z0-9_\-]{16,64})"#).unwrap()),
            ("AWS Access Key ID", Regex::new(r#"(AKIA[0-9A-Z]{16})"#).unwrap()),
            ("Bearer Token", Regex::new(r#"(?i)bearer\s+([a-zA-Z0-9_\-\.]{20,})"#).unwrap()),
        ];
        Self { patterns }
    }

    pub fn scan_text(&self, text: &str) -> Vec<SecretFinding> {
        let mut results = Vec::new();
        for (rule, regex) in &self.patterns {
            for cap in regex.captures_iter(text) {
                if let Some(m) = cap.get(1) {
                    results.push(SecretFinding {
                        rule: rule.to_string(),
                        matched_text: m.as_str().to_string(),
                    });
                }
            }
        }
        results
    }
}

impl Default for SecretScanner {
    fn default() -> Self {
        Self::new()
    }
}
