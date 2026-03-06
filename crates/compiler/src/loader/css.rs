use std::path::Path;

use super::{LoadResult, Loader, escape_css_for_js, style_injection_iife};

pub struct CssLoader;

impl Loader for CssLoader {
    fn extensions(&self) -> &[&str] {
        &["css"]
    }

    fn load(&self, _path: &Path, content: String) -> Result<LoadResult, String> {
        let escaped = escape_css_for_js(&content);
        Ok(LoadResult {
            js_source: style_injection_iife(&escaped),
            needs_transform: false,
            needs_loader_transform: false,
            css_module_exports: None,
        })
    }
}
