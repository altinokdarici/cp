use std::collections::BTreeMap;
use std::path::Path;

use lightningcss::css_modules::Config as CssModulesConfig;
use lightningcss::printer::PrinterOptions;
use lightningcss::stylesheet::{ParserOptions, StyleSheet};

use super::{LoadResult, Loader, escape_css_for_js, style_injection_iife};

pub struct CssModuleLoader;

impl Loader for CssModuleLoader {
    fn extensions(&self) -> &[&str] {
        &["module.css"]
    }

    fn load(&self, _path: &Path, content: String) -> Result<LoadResult, String> {
        // Store raw CSS for Phase 2 transform.
        Ok(LoadResult {
            js_source: content,
            needs_transform: false,
            needs_loader_transform: true,
            css_module_exports: None,
        })
    }

    fn transform(&self, path: &Path, result: &mut LoadResult) -> Result<(), String> {
        let filename = path
            .file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("unknown.module.css")
            .to_string();

        // Take raw CSS out of js_source to avoid borrow conflict with StyleSheet.
        let raw_css = std::mem::take(&mut result.js_source);

        let (scoped_code, exports) = {
            let stylesheet = StyleSheet::parse(
                &raw_css,
                ParserOptions {
                    filename,
                    css_modules: Some(CssModulesConfig::default()),
                    ..Default::default()
                },
            )
            .map_err(|e| format!("CSS parse error in {}: {e}", path.display()))?;

            let css_result = stylesheet
                .to_css(PrinterOptions::default())
                .map_err(|e| format!("CSS codegen error in {}: {e}", path.display()))?;

            (css_result.code, css_result.exports)
        };

        // Build scoped CSS injection IIFE.
        let escaped_css = escape_css_for_js(&scoped_code);
        result.js_source = style_injection_iife(&escaped_css);

        // Build exports map from lightningcss output.
        if let Some(exports) = exports {
            let mut map = BTreeMap::new();
            for (original, export) in exports {
                map.insert(original, export.name);
            }
            result.css_module_exports = Some(map);
        }

        Ok(())
    }
}
