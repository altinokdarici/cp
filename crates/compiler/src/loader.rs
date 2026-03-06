use std::path::Path;

/// Load a file and return JavaScript source code.
/// For TS/JSX files, transformation happens later in the pipeline.
/// For non-JS files, convert to a JS module here.
///
/// Accepts owned String to avoid cloning for the common JS/TS case.
pub fn load(path: &Path, content: String) -> Result<LoadResult, String> {
    let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");

    match ext {
        "ts" | "tsx" | "mts" | "js" | "jsx" | "mjs" => Ok(LoadResult {
            source: content, // Move, not clone.
            needs_transform: matches!(ext, "ts" | "tsx" | "mts" | "jsx"),
        }),
        "json" => Ok(LoadResult {
            source: format!("export default {};", content),
            needs_transform: false,
        }),
        "css" => Ok(LoadResult {
            source: format!(
                "export default {};",
                serde_json::to_string(&content).unwrap()
            ),
            needs_transform: false,
        }),
        "graphql" | "gql" => Ok(LoadResult {
            source: format!(
                "export default {};",
                serde_json::to_string(&content).unwrap()
            ),
            needs_transform: false,
        }),
        "svg" | "txt" | "html" => Ok(LoadResult {
            source: format!(
                "export default {};",
                serde_json::to_string(&content).unwrap()
            ),
            needs_transform: false,
        }),
        _ => Err(format!("Unsupported file type: .{ext}")),
    }
}

pub struct LoadResult {
    pub source: String,
    pub needs_transform: bool,
}
