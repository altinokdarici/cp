use std::path::Path;

use super::{LoadResult, Loader};

pub struct JsLoader;

impl Loader for JsLoader {
    fn extensions(&self) -> &[&str] {
        &["ts", "tsx", "mts", "js", "jsx", "mjs"]
    }

    fn load(&self, path: &Path, content: String) -> Result<LoadResult, String> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        Ok(LoadResult {
            js_source: content,
            needs_transform: matches!(ext, "ts" | "tsx" | "mts" | "jsx"),
            needs_loader_transform: false,
            css_module_exports: None,
        })
    }
}
