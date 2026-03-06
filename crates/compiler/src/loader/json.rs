use std::path::Path;

use super::{LoadResult, Loader};

pub struct JsonLoader;

impl Loader for JsonLoader {
    fn extensions(&self) -> &[&str] {
        &["json"]
    }

    fn load(&self, _path: &Path, content: String) -> Result<LoadResult, String> {
        Ok(LoadResult {
            js_source: format!("export default {};", content),
            needs_transform: false,
            needs_loader_transform: false,
            css_module_exports: None,
        })
    }
}
