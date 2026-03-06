use std::collections::{BTreeMap, HashMap};
use std::path::Path;

mod css;
mod css_module;
mod graphql;
mod js;
mod json;
mod text;

pub use css::CssLoader;
pub use css_module::CssModuleLoader;
pub use graphql::GraphQlLoader;
pub use js::JsLoader;
pub use json::JsonLoader;
pub use text::TextLoader;

/// Result of loading a file through a loader.
pub struct LoadResult {
    pub js_source: String,
    pub needs_transform: bool,
    pub needs_loader_transform: bool,
    pub css_module_exports: Option<BTreeMap<String, String>>,
}

/// Trait for file loaders that convert various file types to JavaScript.
pub trait Loader: Send + Sync {
    /// File extensions this loader handles (without leading dot).
    fn extensions(&self) -> &[&str];

    /// Load a file and return JavaScript source code.
    fn load(&self, path: &Path, content: String) -> Result<LoadResult, String>;

    /// Optional Phase 2 transform (called in parallel for `needs_loader_transform` modules).
    fn transform(&self, _path: &Path, _result: &mut LoadResult) -> Result<(), String> {
        Ok(())
    }
}

/// Registry that maps file extensions to loaders.
pub struct LoaderRegistry {
    loaders: Vec<Box<dyn Loader>>,
    extension_map: HashMap<String, usize>,
}

impl LoaderRegistry {
    pub fn new() -> Self {
        Self {
            loaders: Vec::new(),
            extension_map: HashMap::new(),
        }
    }

    pub fn register(&mut self, loader: Box<dyn Loader>) {
        let idx = self.loaders.len();
        for ext in loader.extensions() {
            self.extension_map.insert(ext.to_string(), idx);
        }
        self.loaders.push(loader);
    }

    pub fn loader_for(&self, path: &Path) -> Option<&dyn Loader> {
        // Try compound extension first (e.g., "module.css" from "button.module.css").
        if let Some(compound) = compound_extension(path)
            && let Some(&idx) = self.extension_map.get(compound)
        {
            return Some(self.loaders[idx].as_ref());
        }
        // Fall back to simple extension.
        let ext = path.extension()?.to_str()?;
        let &idx = self.extension_map.get(ext)?;
        Some(self.loaders[idx].as_ref())
    }
}

/// Extract compound extension (e.g., "module.css" from "button.module.css").
/// Returns a &str slice into the filename to avoid allocation.
fn compound_extension(path: &Path) -> Option<&str> {
    let file_name = path.file_name()?.to_str()?;
    let mut iter = file_name.rsplitn(3, '.');
    let _ext = iter.next()?; // e.g., "css"
    let middle = iter.next()?; // e.g., "module"
    iter.next()?; // stem must exist for a compound extension
    // Return a slice of the original filename: "module.css"
    let start = middle.as_ptr() as usize - file_name.as_ptr() as usize;
    Some(&file_name[start..])
}

/// Escape CSS content for embedding in a JavaScript string literal.
/// Single-pass to avoid intermediate String allocations from chained .replace().
pub fn escape_css_for_js(input: &str) -> String {
    let mut out = String::with_capacity(input.len() + input.len() / 8);
    for c in input.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            _ => out.push(c),
        }
    }
    out
}

/// Format escaped CSS into a runtime style injection IIFE.
pub fn style_injection_iife(escaped_css: &str) -> String {
    format!(
        "(function() {{\n\
         \x20 var style = document.createElement(\"style\");\n\
         \x20 style.textContent = \"{escaped_css}\";\n\
         \x20 document.head.appendChild(style);\n\
         }})();\n"
    )
}

/// Build the default loader registry with all built-in loaders.
pub fn default_registry() -> LoaderRegistry {
    let mut registry = LoaderRegistry::new();
    registry.register(Box::new(CssModuleLoader));
    registry.register(Box::new(JsLoader));
    registry.register(Box::new(JsonLoader));
    registry.register(Box::new(CssLoader));
    registry.register(Box::new(GraphQlLoader));
    registry.register(Box::new(TextLoader));
    registry
}
