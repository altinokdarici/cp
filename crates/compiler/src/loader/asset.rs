use std::path::Path;

use base64::Engine;
use base64::engine::general_purpose::STANDARD as BASE64_STANDARD;

use super::{LoadResult, Loader};

/// Loader for binary assets (images, fonts). Inlines them as base64 data URLs.
pub struct AssetLoader;

/// Extensions handled by the asset loader and their MIME types.
/// Single source of truth — used by both `extensions()` and `load()`.
const ASSET_TYPES: &[(&str, &str)] = &[
    ("png", "image/png"),
    ("jpg", "image/jpeg"),
    ("jpeg", "image/jpeg"),
    ("gif", "image/gif"),
    ("webp", "image/webp"),
    ("avif", "image/avif"),
    ("ico", "image/x-icon"),
    ("bmp", "image/bmp"),
    ("woff", "font/woff"),
    ("woff2", "font/woff2"),
    ("ttf", "font/ttf"),
    ("eot", "application/vnd.ms-fontobject"),
    ("otf", "font/otf"),
];

/// Flattened extension list for the `Loader` trait.
const ASSET_EXTENSIONS: &[&str] = &[
    "png", "jpg", "jpeg", "gif", "webp", "avif", "ico", "bmp", "woff", "woff2", "ttf", "eot", "otf",
];

impl Loader for AssetLoader {
    fn extensions(&self) -> &[&str] {
        ASSET_EXTENSIONS
    }

    fn is_binary(&self) -> bool {
        true
    }

    fn load(&self, path: &Path, _content: String) -> Result<LoadResult, String> {
        let bytes =
            std::fs::read(path).map_err(|e| format!("Failed to read {}: {e}", path.display()))?;
        let mime = mime_from_extension(path);
        let b64 = BASE64_STANDARD.encode(&bytes);

        // Pre-allocate exact capacity: `export default "data:{mime};base64,{b64}";`
        let prefix_len = "export default \"data:".len() + mime.len() + ";base64,".len();
        let mut js_source = String::with_capacity(prefix_len + b64.len() + "\";".len());
        js_source.push_str("export default \"data:");
        js_source.push_str(mime);
        js_source.push_str(";base64,");
        js_source.push_str(&b64);
        js_source.push_str("\";");

        Ok(LoadResult {
            js_source,
            needs_transform: false,
            needs_loader_transform: false,
            css_module_exports: None,
        })
    }
}

fn mime_from_extension(path: &Path) -> &'static str {
    let ext = path
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or_default();
    for &(e, mime) in ASSET_TYPES {
        if e == ext {
            return mime;
        }
    }
    "application/octet-stream"
}
