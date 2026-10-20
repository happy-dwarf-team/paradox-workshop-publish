//! `.metadata/metadata.json`: the JSON descriptor of Victoria 3 and EU5 mods
//! (spec section 5).

use serde::Deserialize;

/// The fields of `metadata.json` that publishing reads. Unknown keys are
/// ignored, because mods and tools add their own.
#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct MetadataJson {
    /// The Workshop title.
    pub name: String,
    /// The modder's stable mod ID (not the Workshop item ID).
    pub id: Option<String>,
    pub version: Option<String>,
    pub game_id: Option<String>,
    pub supported_game_version: Option<String>,
    pub short_description: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
    pub picture: Option<String>,
    pub game_custom_data: Option<GameCustomData>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Deserialize)]
pub struct GameCustomData {
    #[serde(default)]
    pub replace_paths: Vec<String>,
}

impl MetadataJson {
    /// `game_custom_data.replace_paths`, or nothing.
    pub fn replace_paths(&self) -> &[String] {
        self.game_custom_data
            .as_ref()
            .map_or(&[], |data| &data.replace_paths)
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MetadataError {
    #[error("{0}")]
    Json(#[from] serde_json::Error),
    #[error("`name` is empty; the Workshop title comes from it")]
    EmptyName,
}

/// Parses the bytes of a `metadata.json`. An optional UTF-8 BOM is stripped,
/// as pdx-workshop-manager does.
pub fn parse(bytes: &[u8]) -> Result<MetadataJson, MetadataError> {
    let bytes = bytes.strip_prefix(b"\xEF\xBB\xBF").unwrap_or(bytes);
    let metadata: MetadataJson = serde_json::from_slice(bytes)?;
    if metadata.name.trim().is_empty() {
        return Err(MetadataError::EmptyName);
    }
    Ok(metadata)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_known_fields_and_ignores_the_rest() {
        let json = br#"{
            "name": "My Mod",
            "id": "com.example.mymod",
            "version": "1.2.0",
            "game_id": "eu5",
            "supported_game_version": "1.0.*",
            "short_description": "Short",
            "tags": ["Gameplay", "Map"],
            "relationships": [],
            "game_custom_data": { "replace_paths": ["in_game/map_data"], "other": 1 },
            "cmf_last_updated_game_version": "1.0.3"
        }"#;
        let metadata = parse(json).unwrap();
        assert_eq!(metadata.name, "My Mod");
        assert_eq!(metadata.id.as_deref(), Some("com.example.mymod"));
        assert_eq!(metadata.version.as_deref(), Some("1.2.0"));
        assert_eq!(metadata.game_id.as_deref(), Some("eu5"));
        assert_eq!(metadata.tags, ["Gameplay", "Map"]);
        assert_eq!(metadata.replace_paths(), ["in_game/map_data"]);
    }

    #[test]
    fn strips_a_bom() {
        let metadata = parse(b"\xEF\xBB\xBF{\"name\": \"A\"}").unwrap();
        assert_eq!(metadata.name, "A");
        assert!(metadata.tags.is_empty());
        assert!(metadata.replace_paths().is_empty());
    }

    #[test]
    fn name_is_required() {
        let message = parse(b"{\"version\": \"1\"}").unwrap_err().to_string();
        assert!(message.contains("missing field `name`"), "{message}");
        assert!(matches!(
            parse(b"{\"name\": \"\"}"),
            Err(MetadataError::EmptyName)
        ));
    }

    #[test]
    fn json_errors_name_the_line() {
        let message = parse(b"{\n\"name\": \"A\",\n\"tags\": \"x\"\n}")
            .unwrap_err()
            .to_string();
        assert!(message.contains("line 3"), "{message}");
    }
}
