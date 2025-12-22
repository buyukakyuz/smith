pub const MARKER_START: &str = "<!-- __SMITH_INTERNAL__ ";
pub const MARKER_END: &str = " -->";

#[must_use]
pub fn wrap(json: &str) -> String {
    format!("{MARKER_START}{json}{MARKER_END}")
}

#[must_use]
pub fn extract(output: &str) -> Option<&str> {
    let start_idx = output.find(MARKER_START)?;
    let json_start = start_idx + MARKER_START.len();
    let end_idx = output[json_start..].find(MARKER_END)?;
    Some(&output[json_start..json_start + end_idx])
}

#[must_use]
pub fn strip(output: &str) -> String {
    let Some(start_idx) = output.find(MARKER_START) else {
        return output.to_string();
    };

    let Some(end_offset) = output[start_idx..].find(MARKER_END) else {
        return output.to_string();
    };

    let end_idx = start_idx + end_offset + MARKER_END.len();

    let before = output[..start_idx].trim_end();
    let after = output[end_idx..].trim_start();

    if after.is_empty() {
        before.to_string()
    } else {
        format!("{before}\n{after}")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_wrap() {
        let json = r#"{"key":"value"}"#;
        let wrapped = wrap(json);
        assert_eq!(wrapped, r#"<!-- __SMITH_INTERNAL__ {"key":"value"} -->"#);
    }

    #[test]
    fn test_extract() {
        let output = r#"Some output
<!-- __SMITH_INTERNAL__ {"key":"value"} -->"#;
        let json = extract(output);
        assert_eq!(json, Some(r#"{"key":"value"}"#));
    }

    #[test]
    fn test_extract_no_marker() {
        let output = "Just regular output";
        assert_eq!(extract(output), None);
    }

    #[test]
    fn test_strip() {
        let output = r#"Wrote 50 bytes to file.txt
<!-- __SMITH_INTERNAL__ {"key":"value"} -->"#;
        let stripped = strip(output);
        assert_eq!(stripped, "Wrote 50 bytes to file.txt");
    }

    #[test]
    fn test_strip_no_marker() {
        let output = "Just regular output";
        assert_eq!(strip(output), output);
    }

    #[test]
    fn test_strip_with_content_after() {
        let output = r#"Before
<!-- __SMITH_INTERNAL__ {"key":"value"} -->
After"#;
        let stripped = strip(output);
        assert_eq!(stripped, "Before\nAfter");
    }
}
