use std::collections::BTreeMap;
use std::path::PathBuf;

use compiler::{CompileOptions, Manifest, OutputFile};
use depgraph::TraceOptions;

use crate::import_map::{ImportMap, generate_import_map, virtual_prefix};

/// Options for building an application and its dependencies.
pub struct BuildOptions {
    /// Root directory of the application (where package.json lives).
    pub app_root: PathBuf,
    /// Entry file paths relative to app_root.
    pub entries: Vec<PathBuf>,
    /// Whether to generate source maps for output files.
    pub source_maps: bool,
}

/// A compiled package with its metadata.
#[derive(Debug)]
pub struct CompiledPackage {
    /// Package name from package.json (or "_app" for the application).
    pub name: String,
    /// Package version from package.json.
    pub version: String,
    /// Virtual URL prefix, e.g. "/@cp/react@18.2.0/".
    pub virtual_prefix: String,
    /// Compiled output files (JS bundles + shared chunks).
    pub files: Vec<OutputFile>,
    /// Manifest describing entries, chunks, and externals.
    pub manifest: Manifest,
}

/// The complete build output: app, all dependency packages, and the import map.
pub struct BuildOutput {
    /// The compiled application code.
    pub app: CompiledPackage,
    /// All compiled npm dependency packages (direct + transitive).
    pub packages: Vec<CompiledPackage>,
    /// Maps bare specifiers to virtual paths for browser resolution.
    pub import_map: ImportMap,
}

/// Errors that can occur during a build.
#[derive(Debug, thiserror::Error)]
pub enum BuildError {
    #[error("Compile error for {package}: {message}")]
    CompileError { package: String, message: String },
    #[error("Trace error: {0}")]
    TraceError(#[from] depgraph::TraceError),
}

/// Build an application: trace dependency graph, compile the app and each
/// dependency exactly once, and produce an import map.
///
/// Three-phase pipeline:
///   Phase 1: Trace the full dependency graph (parse-only, no compilation).
///   Phase 2: Compile the app and each discovered package exactly once.
///   Phase 3: Generate the import map from all compiled packages.
pub fn build(options: BuildOptions) -> Result<BuildOutput, BuildError> {
    // Phase 1: Trace the full dependency graph (parse-only, parallelized).
    let trace_output = depgraph::trace(TraceOptions {
        app_root: options.app_root.clone(),
        entries: options.entries.clone(),
    })?;

    // Phase 2: Compile the application code.
    let app_output = compiler::compile(CompileOptions {
        package_root: options.app_root.clone(),
        entries: options.entries.clone(),
        source_maps: options.source_maps,
    })
    .map_err(|message| BuildError::CompileError {
        package: "_app".to_string(),
        message,
    })?;

    let app = CompiledPackage {
        name: "_app".to_string(),
        version: "0.0.0".to_string(),
        virtual_prefix: "/@cp/_app/".to_string(),
        files: app_output.files,
        manifest: app_output.manifest,
    };

    // Compile each discovered package exactly once.
    let mut packages: Vec<CompiledPackage> = Vec::with_capacity(trace_output.packages.len());
    let mut specifier_to_entry: BTreeMap<String, (String, usize)> = BTreeMap::new();

    for pkg_info in &trace_output.packages {
        let pkg_key = format!("{}@{}", pkg_info.name, pkg_info.version);

        // Record specifier → (package_key, entry_index) mappings.
        for (idx, specifier) in pkg_info.specifiers.iter().enumerate() {
            specifier_to_entry.insert(specifier.clone(), (pkg_key.clone(), idx));
        }

        let output = compiler::compile(CompileOptions {
            package_root: pkg_info.directory.clone(),
            entries: pkg_info.entries.clone(),
            source_maps: options.source_maps,
        })
        .map_err(|message| BuildError::CompileError {
            package: pkg_key.clone(),
            message,
        })?;

        packages.push(CompiledPackage {
            virtual_prefix: virtual_prefix(&pkg_info.name, &pkg_info.version),
            name: pkg_info.name.clone(),
            version: pkg_info.version.clone(),
            files: output.files,
            manifest: output.manifest,
        });
    }

    // Phase 3: Build the import map.
    let import_map = generate_import_map(&packages, &specifier_to_entry);

    Ok(BuildOutput {
        app,
        packages,
        import_map,
    })
}
