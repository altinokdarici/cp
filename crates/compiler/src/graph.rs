use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};

use oxc_allocator::Allocator;
use oxc_ast::ast::{
    ExportAllDeclaration, ExportNamedDeclaration, ImportDeclaration, ImportDeclarationSpecifier,
    Program, Statement,
};
use oxc_ast_visit::Visit;
use oxc_codegen::{Codegen, CodegenOptions};
use oxc_parser::Parser;
use oxc_resolver::{ResolveOptions, Resolver};
use oxc_semantic::SemanticBuilder;
use oxc_span::SourceType;
use oxc_transformer::{TransformOptions, Transformer};
use rayon::prelude::*;

use crate::loader::{LoadResult, LoaderRegistry};

/// Binding information for CSS module imports in a consuming module.
#[derive(Debug)]
pub struct CssModuleBinding {
    pub css_module_path: PathBuf,
    pub kind: CssModuleBindingKind,
}

/// The kind of import binding used for a CSS module.
#[derive(Debug)]
pub enum CssModuleBindingKind {
    /// `import styles from './button.module.css'`
    Default(String),
    /// `import * as styles from './button.module.css'`
    Namespace(String),
    /// `import { button as btn, header } from './button.module.css'`
    Named(Vec<(String, String)>),
}

/// A module in the package's internal dependency graph.
#[derive(Debug)]
pub struct Module {
    /// Imports that resolve to files within this package (absolute paths).
    pub internal_imports: Vec<PathBuf>,
    /// Imports that resolve outside this package (bare specifiers, kept as-is).
    pub external_imports: Vec<String>,
    /// Transformed JS source.
    pub js_source: String,
    /// Per-module source map (present when source_maps is enabled and codegen ran).
    pub source_map: Option<oxc_sourcemap::SourceMap>,
    /// CSS module scoped class name exports (only for *.module.css files).
    pub css_module_exports: Option<BTreeMap<String, String>>,
    /// CSS module bindings this module imports (binding injected by linker).
    pub css_module_bindings: Vec<CssModuleBinding>,
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
    source: String,
    needs_transform: bool,
    needs_loader_transform: bool,
    internal_imports: Vec<PathBuf>,
    external_imports: Vec<String>,
    source_map: Option<oxc_sourcemap::SourceMap>,
    css_module_bindings: Vec<CssModuleBinding>,
}

/// Build the module graph starting from the given entry file paths.
///
/// Phase 1 (sequential): Read files, parse for import extraction, resolve dependencies.
/// Phase 2 (parallel): Transform TS/JSX and loader transforms across all CPU cores via rayon.
pub fn build_module_graph(
    entries: &[PathBuf],
    package_root: &Path,
    source_maps: bool,
    registry: &LoaderRegistry,
) -> Result<ModuleGraph, String> {
    let canonical_root = package_root.canonicalize().map_err(|e| {
        format!(
            "Failed to canonicalize package root {}: {e}",
            package_root.display()
        )
    })?;

    // Core extensions that the resolver tries for extensionless imports.
    let mut extensions: Vec<String> = vec![
        ".ts".into(),
        ".tsx".into(),
        ".mts".into(),
        ".js".into(),
        ".jsx".into(),
        ".mjs".into(),
        ".json".into(),
        ".css".into(),
    ];
    // Append all loader-registered extensions (assets, etc.) so the resolver
    // can find them without duplicating the list here.
    for ext in registry.extensions() {
        let dotted = format!(".{ext}");
        if !extensions.contains(&dotted) {
            extensions.push(dotted);
        }
    }

    let resolver = Resolver::new(ResolveOptions {
        extensions,
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
            source_maps,
            registry,
        )?;
        resolved_entries.push(resolved);
    }

    // Phase 2: Transform + codegen (for modules needing TS/JSX or loader transform).
    let transform_count = discovered
        .values()
        .filter(|d| d.needs_transform || d.needs_loader_transform)
        .count();
    let modules: HashMap<PathBuf, Module> = if transform_count >= 32 {
        // Parallel path: rayon distributes transform+codegen across CPU cores.
        discovered
            .into_par_iter()
            .map(|(key, disc)| finish_module(key, disc, source_maps, registry))
            .collect::<Result<_, String>>()?
    } else {
        // Sequential path: avoids rayon overhead for small graphs.
        discovered
            .into_iter()
            .map(|(key, disc)| finish_module(key, disc, source_maps, registry))
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
    source_maps: bool,
    registry: &LoaderRegistry,
) -> Result<(), String> {
    // All callers provide canonical paths (resolve_entry canonicalizes, oxc_resolver
    // returns canonical paths), so skip the redundant canonicalize() syscall.
    if discovered.contains_key(abs_path) || visiting.contains(abs_path) {
        return Ok(());
    }

    visiting.insert(abs_path.to_path_buf());

    let loader = registry.loader_for(abs_path).ok_or_else(|| {
        let ext = abs_path.extension().and_then(|e| e.to_str()).unwrap_or("");
        format!("Unsupported file type: .{ext}")
    })?;

    let raw_content = if loader.is_binary() {
        String::new()
    } else {
        std::fs::read_to_string(abs_path)
            .map_err(|e| format!("Failed to read {}: {e}", abs_path.display()))?
    };

    let load_result = loader.load(abs_path, raw_content)?;

    // Binary assets and loader-transform modules have no JS imports to extract — skip parsing.
    if loader.is_binary() || load_result.needs_loader_transform {
        let owned_path = visiting.take(abs_path).unwrap();
        discovered.insert(
            owned_path,
            DiscoveredModule {
                source: load_result.js_source,
                needs_transform: false,
                needs_loader_transform: load_result.needs_loader_transform,
                internal_imports: Vec::new(),
                external_imports: Vec::new(),
                source_map: None,
                css_module_bindings: Vec::new(),
            },
        );
        return Ok(());
    }

    // Parse for import extraction. For non-transform modules, we also strip imports
    // and codegen here to avoid a redundant re-parse in Phase 2.
    let allocator = Allocator::default();
    let source_type = SourceType::from_path(abs_path).unwrap_or_default();
    let mut parsed = Parser::new(&allocator, &load_result.js_source, source_type).parse();

    if parsed.panicked {
        return Err(format!("Parse error in {}", abs_path.display()));
    }

    let mut collector = ImportCollector::default();
    collector.visit_program(&parsed.program);

    let mut internal_imports = Vec::new();
    let mut external_imports = Vec::new();
    let mut css_module_bindings = Vec::new();
    let module_dir = abs_path.parent().unwrap();

    for import in &collector.imports {
        if is_relative_import(&import.specifier) {
            let resolved = resolver
                .resolve(module_dir, &import.specifier)
                .map_err(|e| {
                    format!(
                        "Failed to resolve '{}' from {}: {e}",
                        import.specifier,
                        abs_path.display()
                    )
                })?;

            let resolved_path = resolved.into_path_buf();

            if resolved_path.starts_with(canonical_root) {
                internal_imports.push(resolved_path.clone());

                // Track CSS module bindings for the linker.
                if is_css_module_path(&resolved_path)
                    && let Some(binding) = build_css_module_binding(&resolved_path, import)
                {
                    css_module_bindings.push(binding);
                }

                discover_module(
                    resolver,
                    &resolved_path,
                    canonical_root,
                    discovered,
                    visiting,
                    source_maps,
                    registry,
                )?;
            } else {
                external_imports.push(import.specifier.clone());
            }
        } else {
            external_imports.push(import.specifier.clone());
        }
    }

    // For non-transform modules, strip imports and codegen now (reusing the Phase 1 parse).
    // Transform modules keep raw source for Phase 2 (parallel transform+strip+codegen).
    // Modules with no imports at all skip stripping entirely — use raw source directly.
    let has_imports = !internal_imports.is_empty() || !external_imports.is_empty();
    let (source, source_map) = if !load_result.needs_transform && has_imports {
        // Non-transform module with imports: strip+codegen now, reusing the Phase 1 parse.
        strip_ast_imports(&mut parsed.program);
        if source_maps {
            let options = CodegenOptions {
                source_map_path: Some(abs_path.to_path_buf()),
                ..Default::default()
            };
            let result = Codegen::new()
                .with_options(options)
                .with_source_text(&load_result.js_source)
                .build(&parsed.program);
            (result.code, result.map)
        } else {
            (Codegen::new().build(&parsed.program).code, None)
        }
    } else {
        // Transform modules keep raw source for Phase 2.
        // Modules with no imports need no stripping — use raw source directly.
        (load_result.js_source, None)
    };

    // Recover the owned PathBuf from `visiting` to reuse for `discovered` (avoids extra clone).
    let owned_path = visiting.take(abs_path).unwrap();
    discovered.insert(
        owned_path,
        DiscoveredModule {
            source,
            needs_transform: load_result.needs_transform,
            needs_loader_transform: false,
            internal_imports,
            external_imports,
            source_map,
            css_module_bindings,
        },
    );

    Ok(())
}

/// Convert a discovered module into its final form.
/// Non-transform modules already have their final source from Phase 1.
/// Transform modules go through parse → transform → strip → codegen here.
/// Loader-transform modules call the loader's transform() method.
fn finish_module(
    key: PathBuf,
    disc: DiscoveredModule,
    source_maps: bool,
    registry: &LoaderRegistry,
) -> Result<(PathBuf, Module), String> {
    let (js_source, source_map, css_module_exports) = if disc.needs_transform {
        let (src, map) = transform_and_codegen(&key, &disc.source, source_maps)?;
        (src, map, None)
    } else if disc.needs_loader_transform {
        let loader = registry
            .loader_for(&key)
            .ok_or_else(|| format!("No loader for {}", key.display()))?;
        let mut result = LoadResult {
            js_source: disc.source,
            needs_transform: false,
            needs_loader_transform: true,
            css_module_exports: None,
        };
        loader.transform(&key, &mut result)?;
        (result.js_source, None, result.css_module_exports)
    } else {
        (disc.source, disc.source_map, None)
    };
    Ok((
        key,
        Module {
            internal_imports: disc.internal_imports,
            external_imports: disc.external_imports,
            js_source,
            source_map,
            css_module_exports,
            css_module_bindings: disc.css_module_bindings,
        },
    ))
}

/// Parse → transform TS/JSX → strip import/re-export AST nodes → codegen.
/// Only called for modules that need TS/JSX transformation.
fn transform_and_codegen(
    path: &Path,
    source: &str,
    source_maps: bool,
) -> Result<(String, Option<oxc_sourcemap::SourceMap>), String> {
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

    if source_maps {
        let options = CodegenOptions {
            source_map_path: Some(path.to_path_buf()),
            ..Default::default()
        };
        let codegen_result = Codegen::new()
            .with_options(options)
            .with_source_text(source)
            .build(&parsed.program);
        Ok((codegen_result.code, codegen_result.map))
    } else {
        Ok((Codegen::new().build(&parsed.program).code, None))
    }
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

fn is_css_module_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|n| n.to_str())
        .is_some_and(|n| n.ends_with(".module.css"))
}

fn build_css_module_binding(
    resolved_path: &Path,
    import: &CollectedImport,
) -> Option<CssModuleBinding> {
    let kind = if let Some(ref name) = import.default_binding {
        CssModuleBindingKind::Default(name.clone())
    } else if let Some(ref name) = import.namespace_binding {
        CssModuleBindingKind::Namespace(name.clone())
    } else if !import.named_bindings.is_empty() {
        CssModuleBindingKind::Named(import.named_bindings.clone())
    } else {
        // Side-effect import (import './button.module.css') — no binding needed.
        return None;
    };

    Some(CssModuleBinding {
        css_module_path: resolved_path.to_path_buf(),
        kind,
    })
}

/// A collected import with binding information.
struct CollectedImport {
    specifier: String,
    default_binding: Option<String>,
    namespace_binding: Option<String>,
    named_bindings: Vec<(String, String)>, // (imported, local)
}

/// Collects import specifiers and binding info from the AST.
#[derive(Default)]
struct ImportCollector {
    pub imports: Vec<CollectedImport>,
}

impl<'a> Visit<'a> for ImportCollector {
    fn visit_import_declaration(&mut self, decl: &ImportDeclaration<'a>) {
        let specifier = decl.source.value.to_string();

        // Only collect binding info for potential CSS module imports to avoid
        // thousands of throwaway String allocations for regular JS/TS imports.
        let is_possible_css_module = specifier.ends_with(".module.css");

        let mut import = CollectedImport {
            specifier,
            default_binding: None,
            namespace_binding: None,
            named_bindings: Vec::new(),
        };

        if is_possible_css_module && let Some(specifiers) = &decl.specifiers {
            for spec in specifiers {
                match spec {
                    ImportDeclarationSpecifier::ImportDefaultSpecifier(s) => {
                        import.default_binding = Some(s.local.name.to_string());
                    }
                    ImportDeclarationSpecifier::ImportNamespaceSpecifier(s) => {
                        import.namespace_binding = Some(s.local.name.to_string());
                    }
                    ImportDeclarationSpecifier::ImportSpecifier(s) => {
                        let imported = s.imported.name().to_string();
                        let local = s.local.name.to_string();
                        import.named_bindings.push((imported, local));
                    }
                }
            }
        }

        self.imports.push(import);
    }

    fn visit_export_all_declaration(&mut self, decl: &ExportAllDeclaration<'a>) {
        self.imports.push(CollectedImport {
            specifier: decl.source.value.to_string(),
            default_binding: None,
            namespace_binding: None,
            named_bindings: Vec::new(),
        });
    }

    fn visit_export_named_declaration(&mut self, decl: &ExportNamedDeclaration<'a>) {
        if let Some(source) = &decl.source {
            self.imports.push(CollectedImport {
                specifier: source.value.to_string(),
                default_binding: None,
                namespace_binding: None,
                named_bindings: Vec::new(),
            });
        }
    }
}
