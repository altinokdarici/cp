use std::collections::HashSet;
use std::path::PathBuf;

use serde::Serialize;

use crate::graph;
use crate::linker;

/// Options for compiling a package.
pub struct CompileOptions {
    /// The root directory of the package (where package.json lives).
    pub package_root: PathBuf,
    /// Entry file paths relative to package_root.
    pub entries: Vec<PathBuf>,
}

/// A single output file produced by the compiler.
#[derive(Debug)]
pub struct OutputFile {
    /// Output file name (e.g., "index.js", "chunk-shared.js").
    pub name: String,
    /// The JavaScript content.
    pub content: String,
    /// Optional source map content.
    pub source_map: Option<String>,
}

/// Manifest describing the compilation output.
#[derive(Debug, Serialize)]
pub struct Manifest {
    /// Entry point name → output file name.
    pub entries: Vec<String>,
    /// Shared chunk file names.
    pub chunks: Vec<String>,
    /// External bare specifiers (other packages this one imports).
    pub externals: Vec<String>,
}

/// The complete output of a compilation.
#[derive(Debug)]
pub struct CompileOutput {
    /// All output files (entries + shared chunks).
    pub files: Vec<OutputFile>,
    /// Manifest describing what was produced.
    pub manifest: Manifest,
}

/// Compile a package into ESM bundles.
///
/// 1. Build the internal module graph from entries.
/// 2. Link modules into output files (entries + shared chunks).
/// 3. Return the output files + manifest.
pub fn compile(options: CompileOptions) -> Result<CompileOutput, String> {
    // Step 1: Build module graph.
    let module_graph = graph::build_module_graph(&options.entries, &options.package_root)?;

    // Collect all external imports across the graph (HashSet for O(1) dedup).
    let mut externals_set: HashSet<String> = HashSet::new();
    for module in module_graph.modules.values() {
        for ext in &module.external_imports {
            externals_set.insert(ext.clone());
        }
    }
    let mut externals: Vec<String> = externals_set.into_iter().collect();
    externals.sort();

    // Step 2: Link into output files.
    let files = linker::link(&module_graph, &options.package_root)?;

    // Step 3: Build manifest.
    let mut entries = Vec::new();
    let mut chunks = Vec::new();
    for file in &files {
        if file.name.starts_with("chunk-") {
            chunks.push(file.name.clone());
        } else {
            entries.push(file.name.clone());
        }
    }

    let manifest = Manifest {
        entries,
        chunks,
        externals,
    };

    Ok(CompileOutput { files, manifest })
}
