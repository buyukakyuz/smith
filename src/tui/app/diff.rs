use crate::core::metadata;
pub struct DiffMetadata {
    pub path: String,
    pub old_content: String,
    pub new_content: String,
}

impl DiffMetadata {
    pub fn extract(output: &str) -> Option<Self> {
        let json_str = metadata::extract(output)?;
        let json_value = serde_json::from_str::<serde_json::Value>(json_str).ok()?;
        let diff_obj = json_value.get("diff_metadata")?;

        Some(Self {
            path: diff_obj.get("path")?.as_str()?.to_string(),
            old_content: diff_obj.get("old_content")?.as_str()?.to_string(),
            new_content: diff_obj.get("new_content")?.as_str()?.to_string(),
        })
    }
}
