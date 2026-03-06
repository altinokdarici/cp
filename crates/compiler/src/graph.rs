use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    ExportAllDeclaration, ExportNamedDeclaration, ImportDeclaration, Program, Statement,
};
use oxc_ast_visit::Visit;
use oxc_codegen::Codegen;
use oxc_parser::Parser;
use oxc_resolver::{ResolveOptions, Resolver};
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use oxc_transformer::{TransformOptions, Transformer};
use rayon::prelude::*;

use crate::loader;

/// A module in the package's internal dependency graph.
#[derive(Debug)]
pub struct Module {
    /// Imports that resolve to files within this package (absolute paths).
    pub internal_imports: Vec<PathBuf>,
    /// Imports that resolve outside this package (bare specifiers, kept as-is).
    pub external_imports: Vec<String>,
    /// Transformed JS source.
    pub js_source: String,
}

/// The complete internal module graph for a package.
#[derive(Debug)]
pub struct ModuleGraph {
    /// All modules keyed by absolute path.
    pub modules: HashMap<PathBuf, Module>,
    /// Entry point absolute paths (in the order provided).
    pub entries: Vec<PathBuf>,
}

/// Intermediate state from Phase 1 (discovery). Holds source for Phase 2 (transform).
struct DiscoveredModule {
    path: PathBuf,
    source: String,
    needs_transform: bool,
    internal_imports: Vec<PathBuf>,
    external_imports: Vec<String>,
}

/// Build the module graph starting from the given entry file paths.
///
/// Phase 1 (sequential): Read files, parse for import extraction, resolve dependencies.
/// Phase 2 (parallel): Transform TS/JSX and codegen across all CPU cores via rayon.
pub fn build_module_graph(entries: &[PathBuf], package_root: &Path) -> Result<ModuleGraph, String> {
    let canonical_root = package_root.canonicalize().map_err(|e| {
        format!(
            "Failed to canonicalize package root {}: {e}",
            package_root.display()
        )
    })?;

    let resolver = Resolver::new(ResolveOptions {
        extensions: vec![
            ".ts".into(),
            ".tsx".into(),
            ".mts".into(),
            ".js".into(),
            ".jsx".into(),
            ".mjs".into(),
            ".json".into(),
        ],
        main_fields: vec!["module".into(), "main".into()],
        condition_names: vec!["import".into(), "default".into()],
        ..Default::default()
    });

    // Phase 1: Sequential discovery — read, parse (lightweight), extract imports, resolve.
    let mut discovered: HashMap<PathBuf, DiscoveredModule> = HashMap::new();
    let mut visiting: HashSet<PathBuf> = HashSet::new();
    let mut resolved_entries = Vec::with_capacity(entries.len());

    for entry in entries {
        let abs_entry = if entry.is_absolute() {
            entry.clone()
        } else {
            canonical_root.join(entry)
        };

        let resolved = resolve_entry(&resolver, &abs_entry)?;
        discover_module(
            &resolver,
            &resolved,
            &canonical_root,
            &mut discovered,
            &mut visiting,
        )?;
        resolved_entries.push(resolved);
    }

    // Phase 2: Transform + codegen (only for modules needing TS/JSX transform).
    // Non-transform modules were already stripped+codegen'd in Phase 1.
    let transform_count = discovered.values().filter(|d| d.needs_transform).count();
    let modules: HashMap<PathBuf, Module> = if transform_count >= 32 {
        // Parallel path: rayon distributes transform+codegen across CPU cores.
        discovered
            .into_par_iter()
            .map(|(key, disc)| finish_module(key, disc))
            .collect::<Result<_, String>>()?
    } else {
        // Sequential path: avoids rayon overhead for small graphs.
        discovered
            .into_iter()
            .map(|(key, disc)| finish_module(key, disc))
            .collect::<Result<_, String>>()?
    };

    Ok(ModuleGraph {
        modules,
        entries: resolved_entries,
    })
}

fn resolve_entry(resolver: &Resolver, path: &Path) -> Result<PathBuf, String> {
    if path.is_file() {
        return path
            .canonicalize()
            .map_err(|e| format!("Failed to canonicalize {}: {e}", path.display()));
    }

    let dir = path
        .parent()
        .ok_or_else(|| format!("No parent dir for {}", path.display()))?;
    let file_name = path
        .file_name()
        .ok_or_else(|| format!("No file name for {}", path.display()))?;

    let resolved = resolver
        .resolve(dir, file_name.to_str().unwrap())
        .map_err(|e| format!("Failed to resolve entry {}: {e}", path.display()))?;

    Ok(resolved.into_path_buf())
}

/// Phase 1: Discover a module and its transitive dependencies (sequential DFS).
/// Reads files and parses just enough to extract import specifiers.
fn discover_module(
    resolver: &Resolver,
    abs_path: &Path,
    canonical_root: &Path,
    discovered: &mut HashMap<PathBuf, DiscoveredModule>,
    visiting: &mut HashSet<PathBuf>,
) -> Result<(), String> {
    let canonical = abs_path
        .canonicalize()
        .map_err(|e| format!("Failed to canonicalize {}: {e}", abs_path.display()))?;

    if discovered.contains_key(&canonical) || visiting.contains(&canonical) {
        return Ok(());
    }

    visiting.insert(canonical.clone());

    let raw_content = std::fs::read_to_string(&canonical)
        .map_err(|e| format!("Failed to read {}: {e}", canonical.display()))?;

    let load_result = loader::load(&canonical, raw_content)?;

    // Parse for import extraction. For non-transform modules, we also strip imports
    // and codegen here to avoid a redundant re-parse in Phase 2.
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(&canonical).unwrap_or_default();
    let mut parsed = Parser::new(&allocator, &load_result.source, source_type).parse();

    if parsed.panicked {
        return Err(format!("Parse error in {}", canonical.display()));
    }

    let mut collector = ImportCollector::default();
    collector.visit_program(&parsed.program);

    let mut internal_imports = Vec::new();
    let mut external_imports = Vec::new();
    let module_dir = canonical.parent().unwrap();

    for specifier in &collector.specifiers {
        if is_relative_import(specifier) {
            let resolved = resolver.resolve(module_dir, specifier).map_err(|e| {
                format!(
                    "Failed to resolve '{}' from {}: {e}",
                    specifier,
                    canonical.display()
                )
            })?;

            let resolved_path = resolved.into_path_buf();

            if resolved_path.starts_with(canonical_root) {
                internal_imports.push(resolved_path.clone());
                discover_module(
                    resolver,
                    &resolved_path,
                    canonical_root,
                    discovered,
                    visiting,
                )?;
            } else {
                external_imports.push(specifier.to_string());
            }
        } else {
            external_imports.push(specifier.to_string());
        }
    }

    // For non-transform modules, strip imports and codegen now (reusing the Phase 1 parse).
    // Transform modules keep raw source for Phase 2 (parallel transform+strip+codegen).
    // Modules with no imports at all skip stripping entirely — use raw source directly.
    let has_imports = !internal_imports.is_empty() || !external_imports.is_empty();
    let source = if !load_result.needs_transform && has_imports {
        // Non-transform module with imports: strip+codegen now, reusing the Phase 1 parse.
        strip_ast_imports(&mut parsed.program);
        Codegen::new().build(&parsed.program).code
    } else {
        // Transform modules keep raw source for Phase 2.
        // Modules with no imports need no stripping — use raw source directly.
        load_result.source
    };

    discovered.insert(
        canonical.clone(),
        DiscoveredModule {
            path: canonical.clone(),
            source,
            needs_transform: load_result.needs_transform,
            internal_imports,
            external_imports,
        },
    );

    visiting.remove(&canonical);
    Ok(())
}

/// Convert a discovered module into its final form.
/// Non-transform modules already have their final source from Phase 1.
/// Transform modules go through parse → transform → strip → codegen here.
fn finish_module(key: PathBuf, disc: DiscoveredModule) -> Result<(PathBuf, Module), String> {
    let js_source = if disc.needs_transform {
        transform_and_codegen(&disc.path, &disc.source)?
    } else {
        disc.source
    };
    Ok((
        key,
        Module {
            internal_imports: disc.internal_imports,
            external_imports: disc.external_imports,
            js_source,
        },
    ))
}

/// Parse → transform TS/JSX → strip import/re-export AST nodes → codegen.
/// Only called for modules that need TS/JSX transformation.
fn transform_and_codegen(path: &Path, source: &str) -> Result<String, String> {
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(path).unwrap_or_default();
    let mut parsed = Parser::new(&allocator, source, source_type).parse();

    if parsed.panicked {
        return Err(format!("Parse error in {}", path.display()));
    }

    let semantic_ret = SemanticBuilder::new().build(&parsed.program);
    let scoping = semantic_ret.semantic.into_scoping();
    let transform_options = TransformOptions::default();
    let result = Transformer::new(&allocator, path, &transform_options)
        .build_with_scoping(scoping, &mut parsed.program);

    if !result.errors.is_empty() {
        return Err(format!(
            "Transform errors in {}:\n{}",
            path.display(),
            result
                .errors
                .iter()
                .map(|e: &oxc_diagnostics::OxcDiagnostic| e.to_string())
                .collect::<Vec<_>>()
                .join("\n")
        ));
    }

    strip_ast_imports(&mut parsed.program);

    Ok(Codegen::new().build(&parsed.program).code)
}

/// Remove import declarations and internal re-export-from nodes from the AST.
fn strip_ast_imports(program: &mut Program<'_>) {
    program.body.retain(|stmt| match stmt {
        // Remove ALL import declarations (the linker re-adds externals).
        Statement::ImportDeclaration(_) => false,
        // Remove internal `export * from './...'` re-exports; keep external ones.
        Statement::ExportAllDeclaration(decl) => !is_relative_import(decl.source.value.as_str()),
        // Remove internal `export { x } from './...'` re-exports; keep direct exports and external re-exports.
        Statement::ExportNamedDeclaration(decl) => match &decl.source {
            Some(source) => !is_relative_import(source.value.as_str()),
            None => true,
        },
        _ => true,
    });
}

fn is_relative_import(specifier: &str) -> bool {
    specifier.starts_with("./") || specifier.starts_with("../")
}

/// Collects import specifiers from the AST.
#[derive(Default)]
struct ImportCollector {
    pub specifiers: Vec<String>,
}

impl<'a> Visit<'a> for ImportCollector {
    fn visit_import_declaration(&mut self, decl: &ImportDeclaration<'a>) {
        self.specifiers.push(decl.source.value.to_string());
    }

    fn visit_export_all_declaration(&mut self, decl: &ExportAllDeclaration<'a>) {
        self.specifiers.push(decl.source.value.to_string());
    }

    fn visit_export_named_declaration(&mut self, decl: &ExportNamedDeclaration<'a>) {
        if let Some(source) = &decl.source {
            self.specifiers.push(source.value.to_string());
        }
    }
}
