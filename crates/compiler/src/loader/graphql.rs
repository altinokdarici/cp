use std::path::Path;

use super::{LoadResult, Loader};

pub struct GraphQlLoader;

impl Loader for GraphQlLoader {
    fn extensions(&self) -> &[&str] {
        &["graphql", "gql"]
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
