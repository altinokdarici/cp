use std::path::Path;

use super::{LoadResult, Loader};

pub struct TextLoader;

impl Loader for TextLoader {
    fn extensions(&self) -> &[&str] {
        &["svg", "txt", "html"]
    }

    fn load(&self, path: &Path, content: String) -> Result<LoadResult, String> {
        let json = serde_json::to_string(&content)
            .map_err(|e| format!("Failed to serialize {}: {e}", path.display()))?;
        Ok(LoadResult {
            js_source: format!("export default {};", json),
            needs_transform: false,
            needs_loader_transform: false,
            css_module_exports: None,
        })
    }
}
