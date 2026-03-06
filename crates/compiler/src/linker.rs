use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use crate::compiler::OutputFile;
use crate::graph::ModuleGraph;

/// Determines which modules go into which output files (entries vs shared chunks).
struct ChunkPlan {
    entry_modules: HashMap<PathBuf, Vec<PathBuf>>,
    shared_chunks: Vec<(String, Vec<PathBuf>)>,
}

/// Link a module graph into output files.
///
/// All modules are already transformed to JS in the graph phase.
/// The linker only needs to: compute chunks, strip imports, concatenate, emit.
pub fn link(
    graph: &ModuleGraph,
    package_root: &Path,
    source_maps: bool,
) -> Result<Vec<OutputFile>, String> {
    let canonical_root = package_root
        .canonicalize()
        .unwrap_or(package_root.to_path_buf());

    let plan = compute_chunk_plan(graph)?;
    let mut outputs = Vec::with_capacity(plan.shared_chunks.len() + graph.entries.len());

    // Build shared chunks first.
    for (chunk_name, module_paths) in &plan.shared_chunks {
        let (mut output, raw_map) = build_output_file(
            &format!("{chunk_name}.js"),
            module_paths,
            graph,
            source_maps,
        )?;
        finalize_source_map(&mut output, raw_map);
        outputs.push(output);
    }

    // Build entry files, deduplicating output names.
    let mut name_counts: HashMap<String, usize> = HashMap::new();
    for entry_path in &graph.entries {
        let base_name = entry_name_from_path(entry_path, &canonical_root);
        let count = name_counts.entry(base_name.clone()).or_insert(0);
        let entry_name = if *count == 0 {
            base_name.clone()
        } else {
            format!("{base_name}-{count}")
        };
        *count += 1;

        let exclusive = plan
            .entry_modules
            .get(entry_path)
            .map(|v| v.as_slice())
            .unwrap_or(&[]);

        let (mut output, raw_map) = build_entry_file(
            &format!("{entry_name}.js"),
            entry_path,
            exclusive,
            &plan.shared_chunks,
            graph,
            source_maps,
        )?;
        finalize_source_map(&mut output, raw_map);
        outputs.push(output);
    }

    Ok(outputs)
}

fn compute_chunk_plan(graph: &ModuleGraph) -> Result<ChunkPlan, String> {
    let entry_set: HashSet<&PathBuf> = graph.entries.iter().collect();
    let mut module_entry_count: HashMap<&PathBuf, Vec<usize>> = HashMap::new();

    for (entry_idx, entry) in graph.entries.iter().enumerate() {
        let mut visited = HashSet::with_capacity(graph.modules.len());
        collect_reachable(entry, graph, &mut visited);
        for module_path in visited {
            module_entry_count
                .entry(module_path)
                .or_insert_with(|| Vec::with_capacity(2))
                .push(entry_idx);
        }
    }

    let mut shared_modules: Vec<PathBuf> = Vec::new();
    let mut entry_modules: HashMap<PathBuf, Vec<PathBuf>> = HashMap::new();

    for entry in &graph.entries {
        entry_modules.insert(entry.clone(), Vec::new());
    }

    for (module_path, entries) in &module_entry_count {
        if entries.len() > 1 {
            if !entry_set.contains(module_path) {
                shared_modules.push((*module_path).clone());
            }
        } else {
            let entry = &graph.entries[entries[0]];
            if *module_path != entry {
                entry_modules
                    .entry(entry.clone())
                    .or_default()
                    .push((*module_path).clone());
            }
        }
    }

    let shared_chunks = if shared_modules.is_empty() {
        vec![]
    } else {
        shared_modules.sort();
        vec![("chunk-shared".to_string(), shared_modules)]
    };

    Ok(ChunkPlan {
        entry_modules,
        shared_chunks,
    })
}

fn collect_reachable<'a>(
    module_path: &'a PathBuf,
    graph: &'a ModuleGraph,
    visited: &mut HashSet<&'a PathBuf>,
) {
    if !visited.insert(module_path) {
        return;
    }
    if let Some(module) = graph.modules.get(module_path) {
        for dep in &module.internal_imports {
            collect_reachable(dep, graph, visited);
        }
    }
}

fn build_output_file(
    output_name: &str,
    module_paths: &[PathBuf],
    graph: &ModuleGraph,
    source_maps: bool,
) -> Result<(OutputFile, Option<oxc_sourcemap::SourceMap>), String> {
    // Dedup external imports using HashSet (O(1) lookup instead of Vec::contains).
    let mut seen_imports: HashSet<&str> = HashSet::new();
    let mut all_external_imports: Vec<&str> = Vec::new();
    let mut module_sources: Vec<&str> = Vec::with_capacity(module_paths.len());

    for module_path in module_paths {
        let module = graph
            .modules
            .get(module_path)
            .ok_or_else(|| format!("Module not found: {}", module_path.display()))?;

        for ext in &module.external_imports {
            if seen_imports.insert(ext.as_str()) {
                all_external_imports.push(ext.as_str());
            }
        }

        // js_source already has imports stripped at the AST level in the graph phase.
        module_sources.push(&module.js_source);
    }

    // Estimate output size to pre-allocate.
    let body_size: usize = module_sources.iter().map(|s| s.len() + 1).sum();
    let imports_size: usize = all_external_imports.iter().map(|s| s.len() + 30).sum();
    let mut output = String::with_capacity(body_size + imports_size);

    for ext_import in &all_external_imports {
        output.push_str("import * as _ext_");
        push_safe_identifier(&mut output, ext_import);
        output.push_str(" from \"");
        output.push_str(ext_import);
        output.push_str("\";\n");
    }

    if !all_external_imports.is_empty() {
        output.push('\n');
    }

    for body in &module_sources {
        output.push_str(body);
        output.push('\n');
    }

    // Build combined source map from per-module maps.
    let combined_map = if source_maps {
        let mut line_offset: u32 = all_external_imports.len() as u32;
        if !all_external_imports.is_empty() {
            line_offset += 1; // blank separator line
        }

        let mut pairs: Vec<(&oxc_sourcemap::SourceMap, u32)> = Vec::new();

        for (i, module_path) in module_paths.iter().enumerate() {
            let module = graph.modules.get(module_path).unwrap();
            if let Some(ref sm) = module.source_map {
                pairs.push((sm, line_offset));
            }
            let lines_in_body = module_sources[i].bytes().filter(|b| *b == b'\n').count() as u32;
            line_offset += lines_in_body + 1; // +1 for the \n we append
        }

        if !pairs.is_empty() {
            Some(oxc_sourcemap::ConcatSourceMapBuilder::from_sourcemaps(&pairs).into_sourcemap())
        } else {
            None
        }
    } else {
        None
    };

    Ok((
        OutputFile {
            name: output_name.to_string(),
            content: output,
            source_map: None,
        },
        combined_map,
    ))
}

fn build_entry_file(
    output_name: &str,
    entry_path: &Path,
    exclusive_modules: &[PathBuf],
    shared_chunks: &[(String, Vec<PathBuf>)],
    graph: &ModuleGraph,
    source_maps: bool,
) -> Result<(OutputFile, Option<oxc_sourcemap::SourceMap>), String> {
    let mut all_paths: Vec<PathBuf> = exclusive_modules.to_vec();
    all_paths.push(entry_path.to_path_buf());

    let (mut output_file, raw_map) =
        build_output_file(output_name, &all_paths, graph, source_maps)?;

    // Prepend chunk imports without extra allocation via format!.
    if !shared_chunks.is_empty() {
        let mut with_chunks = String::new();
        let mut prepended_lines: u32 = 0;
        for (chunk_name, _) in shared_chunks {
            with_chunks.push_str("import \"./");
            with_chunks.push_str(chunk_name);
            with_chunks.push_str(".js\";\n");
            prepended_lines += 1;
        }
        with_chunks.push('\n');
        prepended_lines += 1; // blank separator
        with_chunks.push_str(&output_file.content);
        output_file.content = with_chunks;

        // Adjust source map offset for prepended chunk import lines.
        if let Some(map) = raw_map {
            let adjusted =
                oxc_sourcemap::ConcatSourceMapBuilder::from_sourcemaps(&[(&map, prepended_lines)])
                    .into_sourcemap();
            return Ok((output_file, Some(adjusted)));
        }

        return Ok((output_file, None));
    }

    Ok((output_file, raw_map))
}

/// Convert a raw SourceMap to JSON string and append sourceMappingURL comment.
fn finalize_source_map(output: &mut OutputFile, raw_map: Option<oxc_sourcemap::SourceMap>) {
    if let Some(map) = raw_map {
        output.source_map = Some(map.to_json_string());
        output
            .content
            .push_str(&format!("//# sourceMappingURL={}.map\n", output.name));
    }
}

fn entry_name_from_path(entry_path: &Path, canonical_root: &Path) -> String {
    let relative = entry_path
        .strip_prefix(canonical_root)
        .unwrap_or(entry_path);

    let stem = relative
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("index");

    // If the file stem is "index", use the parent directory name to disambiguate.
    // e.g., src/appChrome/index.ts -> appChrome
    if stem == "index"
        && let Some(parent) = relative.parent().and_then(|p| p.file_name())
        && let Some(name) = parent.to_str()
        && name != "src"
    {
        return name.to_string();
    }

    stem.to_string()
}

/// Write a safe identifier directly to output, avoiding a String allocation.
fn push_safe_identifier(output: &mut String, specifier: &str) {
    for c in specifier.chars() {
        if c.is_alphanumeric() {
            output.push(c);
        } else {
            output.push('_');
        }
    }
}
