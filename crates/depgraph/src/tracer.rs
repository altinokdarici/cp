use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use oxc_allocator::Allocator;
use oxc_ast_visit::Visit;
use oxc_parser::Parser;
use oxc_resolver::Resolver;
use oxc_span::SourceType;
use rayon::prelude::*;

use crate::collector::ImportCollector;
use crate::resolver::{
    create_resolver, has_js_imports, is_relative_import, resolve_entry, resolve_specifier,
};
use crate::{PackageInfo, TraceError, TraceOptions, TraceOutput};

/// Result of parsing a single file: its internal and external imports.
struct ParseResult {
    internal_imports: Vec<PathBuf>,
    external_imports: Vec<String>,
}

impl ParseResult {
    fn empty() -> Self {
        Self {
            internal_imports: Vec::new(),
            external_imports: Vec::new(),
        }
    }
}

/// Trace the full dependency graph starting from an application's entry points.
///
/// 1. Traces the app to discover its external imports (parse-only, no compilation).
/// 2. Resolves externals transitively using a BFS with parallel package tracing.
pub fn trace(options: TraceOptions) -> Result<TraceOutput, TraceError> {
    let resolver = create_resolver();

    // Trace the app to discover its bare-specifier externals.
    let app_externals = trace_package(&resolver, &options.app_root, &options.entries)?;

    // Cross-package BFS: resolve externals, trace each new package, repeat.
    let mut packages_map: HashMap<PathBuf, PackageInfo> = HashMap::new();
    // Track resolved directories to avoid tracing the same package twice.
    let mut seen_dirs: HashSet<PathBuf> = HashSet::new();

    // Seed with app externals, resolved from app_root.
    let mut pending: Vec<(String, PathBuf)> = app_externals
        .iter()
        .map(|s| (s.clone(), options.app_root.clone()))
        .collect();

    while !pending.is_empty() {
        let batch = std::mem::take(&mut pending);

        // Resolve all specifiers in this batch, accumulate entries per package,
        // and collect (pkg_dir, entries_to_trace) for newly discovered packages.
        let mut new_traces: Vec<(PathBuf, Vec<PathBuf>)> = Vec::new();

        for (specifier, resolve_from) in batch {
            let resolved = resolve_specifier(&resolver, &resolve_from, &specifier)?;

            // resolved.directory is already canonical from resolve_specifier.
            let canonical_dir = &resolved.directory;

            let pkg = packages_map
                .entry(canonical_dir.clone())
                .or_insert_with(|| PackageInfo {
                    name: resolved.name.clone(),
                    version: resolved.version.clone(),
                    directory: resolved.directory.clone(),
                    entries: Vec::new(),
                    specifiers: Vec::new(),
                });

            // Deduplicate entries within the same package.
            if !pkg.entries.contains(&resolved.entry_relative) {
                pkg.entries.push(resolved.entry_relative.clone());
                pkg.specifiers.push(specifier);

                // If this is a brand-new package directory, queue it for tracing.
                if seen_dirs.insert(canonical_dir.clone()) {
                    new_traces.push((canonical_dir.clone(), vec![resolved.entry_relative]));
                } else {
                    // Package already traced — trace just the new entry to find its externals.
                    new_traces.push((canonical_dir.clone(), vec![resolved.entry_relative]));
                }
            }
            // If the entry is already known, skip — the specifier is a duplicate
            // from a different resolve_from and doesn't add new information.
        }

        // Trace all new entries — parallel if 4+ packages, sequential otherwise.
        let all_externals: Vec<Vec<String>> = if new_traces.len() >= 4 {
            new_traces
                .par_iter()
                .map(|(pkg_dir, entries)| trace_package(&resolver, pkg_dir, entries))
                .collect::<Result<_, _>>()?
        } else {
            new_traces
                .iter()
                .map(|(pkg_dir, entries)| trace_package(&resolver, pkg_dir, entries))
                .collect::<Result<_, _>>()?
        };

        // Enqueue newly discovered externals.
        for (externals, (pkg_dir, _)) in all_externals.into_iter().zip(&new_traces) {
            for ext in externals {
                pending.push((ext, pkg_dir.clone()));
            }
        }
    }

    let packages: Vec<PackageInfo> = packages_map.into_values().collect();

    Ok(TraceOutput { packages })
}

/// Trace a single package using wavefront BFS.
/// Returns the list of external (bare specifier) imports discovered.
fn trace_package(
    resolver: &Resolver,
    package_root: &Path,
    entries: &[PathBuf],
) -> Result<Vec<String>, TraceError> {
    let canonical_root = package_root
        .canonicalize()
        .map_err(|e| TraceError::IoError {
            path: package_root.display().to_string(),
            message: e.to_string(),
        })?;

    let mut discovered: HashSet<PathBuf> = HashSet::new();
    let mut externals: HashSet<String> = HashSet::new();

    // Resolve entry paths.
    let mut wave: Vec<PathBuf> = Vec::with_capacity(entries.len());
    for entry in entries {
        let abs_entry = if entry.is_absolute() {
            entry.clone()
        } else {
            canonical_root.join(entry)
        };
        let resolved = resolve_entry(resolver, &abs_entry)?;
        if discovered.insert(resolved.clone()) {
            wave.push(resolved);
        }
    }

    // Wavefront BFS: parse each wave in parallel when large enough.
    while !wave.is_empty() {
        let results: Vec<ParseResult> = if wave.len() >= 8 {
            wave.par_iter()
                .map(|p| parse_and_resolve(resolver, p, &canonical_root))
                .collect::<Result<_, _>>()?
        } else {
            wave.iter()
                .map(|p| parse_and_resolve(resolver, p, &canonical_root))
                .collect::<Result<_, _>>()?
        };

        let mut next_wave = Vec::with_capacity(wave.len() * 2);
        for result in results {
            externals.extend(result.external_imports);
            for imp in result.internal_imports {
                if discovered.insert(imp.clone()) {
                    next_wave.push(imp);
                }
            }
        }
        wave = next_wave;
    }

    Ok(externals.into_iter().collect())
}

/// Parse a single file and resolve its import specifiers.
fn parse_and_resolve(
    resolver: &Resolver,
    abs_path: &Path,
    canonical_root: &Path,
) -> Result<ParseResult, TraceError> {
    // Non-JS files (json, css, svg, etc.) have no import statements.
    if !has_js_imports(abs_path) {
        return Ok(ParseResult::empty());
    }

    let source = std::fs::read_to_string(abs_path).map_err(|e| TraceError::IoError {
        path: abs_path.display().to_string(),
        message: e.to_string(),
    })?;

    let allocator = Allocator::default();
    let source_type = SourceType::from_path(abs_path).unwrap_or_default();
    let parsed = Parser::new(&allocator, &source, source_type).parse();

    if parsed.panicked {
        return Err(TraceError::ParseError {
            path: abs_path.display().to_string(),
            message: "Parser panicked".to_string(),
        });
    }

    let mut collector = ImportCollector::default();
    collector.visit_program(&parsed.program);
    // allocator + AST dropped here — minimal memory footprint

    let module_dir = abs_path.parent().ok_or_else(|| TraceError::ParseError {
        path: abs_path.display().to_string(),
        message: "File has no parent directory".to_string(),
    })?;
    let mut internal = Vec::new();
    let mut external = Vec::new();

    for specifier in collector.specifiers {
        if is_relative_import(&specifier) {
            let resolved =
                resolver
                    .resolve(module_dir, &specifier)
                    .map_err(|e| TraceError::ResolveError {
                        specifier: specifier.clone(),
                        message: e.to_string(),
                    })?;

            let resolved_path = resolved.into_path_buf();
            if resolved_path.starts_with(canonical_root) {
                internal.push(resolved_path);
            } else {
                // Relative import that escapes the package root → treat as external.
                external.push(specifier);
            }
        } else {
            external.push(specifier);
        }
    }

    Ok(ParseResult {
        internal_imports: internal,
        external_imports: external,
    })
}
