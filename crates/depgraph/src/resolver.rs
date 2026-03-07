use std::path::{Path, PathBuf};

use oxc_resolver::{ResolveOptions, Resolver};

use crate::TraceError;

/// Metadata extracted from resolving a bare specifier to a package on disk.
pub struct ResolvedPackage {
    pub name: String,
    pub version: String,
    /// The directory containing package.json (the package root).
    pub directory: PathBuf,
    /// The resolved file path relative to the package root.
    pub entry_relative: PathBuf,
}

/// Create an oxc_resolver with the standard config for import tracing.
pub fn create_resolver() -> Resolver {
    Resolver::new(ResolveOptions {
        extensions: vec![
            ".ts".into(),
            ".tsx".into(),
            ".mts".into(),
            ".js".into(),
            ".jsx".into(),
            ".mjs".into(),
            ".json".into(),
            ".css".into(),
        ],
        main_fields: vec!["module".into(), "main".into()],
        condition_names: vec!["import".into(), "default".into()],
        ..Default::default()
    })
}

/// Check if a specifier is a relative import (./ or ../).
pub fn is_relative_import(specifier: &str) -> bool {
    specifier.starts_with("./") || specifier.starts_with("../")
}

/// Check if a file extension indicates JS-like content that may contain imports.
pub fn has_js_imports(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .is_some_and(|ext| matches!(ext, "ts" | "tsx" | "mts" | "js" | "jsx" | "mjs"))
}

/// Resolve an entry path to its canonical location on disk.
pub fn resolve_entry(resolver: &Resolver, abs_path: &Path) -> Result<PathBuf> {
    if abs_path.is_file() {
        return abs_path.canonicalize().map_err(|e| TraceError::IoError {
            path: abs_path.display().to_string(),
            message: e.to_string(),
        });
    }

    let dir = abs_path.parent().ok_or_else(|| TraceError::ResolveError {
        specifier: abs_path.display().to_string(),
        message: "No parent directory".to_string(),
    })?;
    let file_name = abs_path
        .file_name()
        .ok_or_else(|| TraceError::ResolveError {
            specifier: abs_path.display().to_string(),
            message: "No file name".to_string(),
        })?;

    let resolved = resolver
        .resolve(dir, file_name.to_str().unwrap())
        .map_err(|e| TraceError::ResolveError {
            specifier: abs_path.display().to_string(),
            message: e.to_string(),
        })?;

    Ok(resolved.into_path_buf())
}

/// Resolve a bare specifier from a directory and extract package metadata.
pub fn resolve_specifier(
    resolver: &Resolver,
    resolve_from: &Path,
    specifier: &str,
) -> Result<ResolvedPackage> {
    let resolution =
        resolver
            .resolve(resolve_from, specifier)
            .map_err(|e| TraceError::ResolveError {
                specifier: specifier.to_string(),
                message: e.to_string(),
            })?;

    let pkg_json = resolution
        .package_json()
        .ok_or_else(|| TraceError::PackageMetadataError {
            path: resolution.path().display().to_string(),
            message: "No package.json found for resolved module".to_string(),
        })?;

    let name = pkg_json
        .name()
        .ok_or_else(|| TraceError::PackageMetadataError {
            path: pkg_json.path.display().to_string(),
            message: "package.json missing 'name' field".to_string(),
        })?
        .to_string();

    let version = pkg_json
        .version()
        .ok_or_else(|| TraceError::PackageMetadataError {
            path: pkg_json.path.display().to_string(),
            message: "package.json missing 'version' field".to_string(),
        })?
        .to_string();

    let directory = pkg_json
        .path
        .parent()
        .ok_or_else(|| TraceError::PackageMetadataError {
            path: pkg_json.path.display().to_string(),
            message: "package.json has no parent directory".to_string(),
        })?
        .canonicalize()
        .map_err(|e| TraceError::IoError {
            path: pkg_json.path.display().to_string(),
            message: e.to_string(),
        })?;

    let full_path = resolution.into_path_buf();
    let entry_relative = full_path
        .strip_prefix(&directory)
        .map_err(|_| TraceError::ResolveError {
            specifier: specifier.to_string(),
            message: format!(
                "Resolved path {} is not under package directory {}",
                full_path.display(),
                directory.display()
            ),
        })?
        .to_path_buf();

    Ok(ResolvedPackage {
        name,
        version,
        directory,
        entry_relative,
    })
}

pub type Result<T> = std::result::Result<T, TraceError>;
