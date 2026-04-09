/// Extract JSON from a response that may contain markdown code fences or surrounding text.
pub fn extract_json(raw: &str) -> Option<String> {
    let trimmed = raw.trim();

    if serde_json::from_str::<serde_json::Value>(trimmed).is_ok() {
        return Some(trimmed.to_string());
    }

    if let Some(start) = trimmed.find("```json") {
        let after = &trimmed[start + 7..];
        if let Some(end) = after.find("```") {
            let json_str = after[..end].trim();
            if serde_json::from_str::<serde_json::Value>(json_str).is_ok() {
                return Some(json_str.to_string());
            }
        }
    }

    if let Some(start) = trimmed.find("```") {
        let after = &trimmed[start + 3..];
        let after = if let Some(nl) = after.find('\n') {
            &after[nl + 1..]
        } else {
            after
        };
        if let Some(end) = after.find("```") {
            let json_str = after[..end].trim();
            if serde_json::from_str::<serde_json::Value>(json_str).is_ok() {
                return Some(json_str.to_string());
            }
        }
    }

    for (open, close) in [("{", "}"), ("[", "]")] {
        if let Some(start) = trimmed.find(open)
            && let Some(end) = trimmed.rfind(close)
            && end > start
        {
            let candidate = &trimmed[start..=end];
            if serde_json::from_str::<serde_json::Value>(candidate).is_ok() {
                return Some(candidate.to_string());
            }
        }
    }

    None
}

/// Embed a JSON schema into a system prompt for providers that don't support structured output natively.
pub fn embed_schema(system_prompt: &str, json_schema: Option<&str>) -> String {
    match json_schema {
        Some(schema) => format!(
            "{system_prompt}\n\n\
             You MUST respond with valid JSON matching this schema:\n\
             ```json\n{schema}\n```\n\n\
             Respond ONLY with the JSON object, no markdown fences, no explanation."
        ),
        None => system_prompt.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_direct_json() {
        let input = r#"{"commits": []}"#;
        assert_eq!(extract_json(input), Some(input.to_string()));
    }

    #[test]
    fn extract_from_json_fences() {
        let input = "Here:\n```json\n{\"commits\": []}\n```\nDone.";
        assert_eq!(extract_json(input), Some(r#"{"commits": []}"#.to_string()));
    }

    #[test]
    fn extract_from_surrounding_text() {
        let input = "Result is {\"commits\": []} done.";
        assert_eq!(extract_json(input), Some(r#"{"commits": []}"#.to_string()));
    }

    #[test]
    fn extract_returns_none_for_invalid() {
        assert_eq!(extract_json("no json here"), None);
        assert_eq!(extract_json(""), None);
    }

    #[test]
    fn embed_schema_passthrough() {
        assert_eq!(embed_schema("Hello", None), "Hello");
    }

    #[test]
    fn embed_schema_injects() {
        let result = embed_schema("Base.", Some(r#"{"type": "object"}"#));
        assert!(result.contains("You MUST respond with valid JSON"));
    }
}
