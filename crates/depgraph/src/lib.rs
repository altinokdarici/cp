mod collector;
pub mod resolver;
mod tracer;

use std::path::PathBuf;

pub use tracer::trace;

/// Options for tracing the dependency graph of an application.
#[derive(Debug)]
pub struct TraceOptions {
    /// Root directory of the application (where package.json lives).
    pub app_root: PathBuf,
    /// Entry file paths (relative to app_root or absolute).
    pub entries: Vec<PathBuf>,
}

/// The result of tracing: all discovered npm packages.
#[derive(Debug)]
pub struct TraceOutput {
    /// All transitively discovered npm packages.
    pub packages: Vec<PackageInfo>,
}

/// Metadata for a discovered npm package.
#[derive(Debug)]
pub struct PackageInfo {
    /// Package name from package.json.
    pub name: String,
    /// Package version from package.json.
    pub version: String,
    /// The directory containing package.json (the package root).
    pub directory: PathBuf,
    /// Entry file paths relative to `directory`, one per specifier.
    pub entries: Vec<PathBuf>,
    /// Bare specifiers that resolved to this package, 1:1 with `entries`.
    pub specifiers: Vec<String>,
}

/// Errors that can occur during dependency tracing.
#[derive(Debug, thiserror::Error)]
pub enum TraceError {
    #[error("Failed to resolve '{specifier}': {message}")]
    ResolveError { specifier: String, message: String },
    #[error("Failed to parse {path}: {message}")]
    ParseError { path: String, message: String },
    #[error("Package metadata error for {path}: {message}")]
    PackageMetadataError { path: String, message: String },
    #[error("IO error reading {path}: {message}")]
    IoError { path: String, message: String },
}
