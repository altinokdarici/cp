use std::path::PathBuf;
use std::time::Instant;

use compiler::{CompileOptions, compile};

struct BenchResult {
    name: String,
    value_ns: u128,
}

/// Generate a realistic package with N modules, M entries, and external imports.
fn generate_package(root: &std::path::Path, num_modules: usize, num_entries: usize) {
    std::fs::create_dir_all(root.join("src")).unwrap();

    std::fs::write(
        root.join("package.json"),
        r#"{"name": "@bench/large-pkg", "version": "1.0.0", "type": "module"}"#,
    )
    .unwrap();

    // Shared utility modules - each one imports from a few others.
    for i in 0..num_modules {
        let mut source = String::new();

        // External imports (simulating real packages).
        match i % 5 {
            0 => source.push_str("import React from 'react';\n"),
            1 => source.push_str("import { css } from '@emotion/css';\n"),
            2 => source.push_str("import { tokens } from '@fluentui/tokens';\n"),
            3 => source.push_str("import lodash from 'lodash';\n"),
            _ => {}
        }

        // Internal imports - import from a few earlier modules (no cycles).
        if i > 0 {
            let dep1 = (i - 1) % num_modules;
            source.push_str(&format!("import {{ util{dep1} }} from './util{dep1}';\n"));
        }
        if i > 3 {
            let dep2 = (i - 3) % num_modules;
            source.push_str(&format!("import {{ util{dep2} }} from './util{dep2}';\n"));
        }

        // TypeScript interface and function body.
        source.push_str(&format!(
            r#"
interface Props{i} {{
    value: string;
    count: number;
    enabled: boolean;
}}

export function util{i}(props: Props{i}): string {{
    const result: string = props.value.repeat(props.count);
    const flag: boolean = props.enabled && result.length > 0;
    return flag ? result.toUpperCase() : result.toLowerCase();
}}

export const CONSTANT_{i}: number = {i};
export type Util{i}Type = ReturnType<typeof util{i}>;
"#
        ));

        std::fs::write(root.join(format!("src/util{i}.ts")), source).unwrap();
    }

    // Entry modules - each imports a spread of utility modules.
    for e in 0..num_entries {
        let mut source = String::new();
        source.push_str("import React from 'react';\n");

        // Each entry imports a different spread of utilities.
        let start = (e * num_modules / num_entries) % num_modules;
        let count = (num_modules / num_entries).max(3);
        for j in 0..count {
            let idx = (start + j) % num_modules;
            source.push_str(&format!("import {{ util{idx} }} from './util{idx}';\n"));
        }

        source.push_str(&format!(
            r#"
interface Entry{e}Props {{
    name: string;
}}

export function Entry{e}(props: Entry{e}Props): React.ReactElement {{
    return React.createElement('div', null, props.name);
}}
"#
        ));

        std::fs::write(root.join(format!("src/entry{e}.ts")), source).unwrap();
    }
}

fn bench(label: &str, iterations: u32, f: impl Fn()) -> BenchResult {
    // Warmup.
    f();

    let start = Instant::now();
    for _ in 0..iterations {
        f();
    }
    let elapsed = start.elapsed();
    let per_iter = elapsed / iterations;

    println!("{label}: {per_iter:?} per iteration ({iterations} iterations, {elapsed:?} total)");

    BenchResult {
        name: label.to_string(),
        value_ns: per_iter.as_nanos(),
    }
}

fn main() {
    let temp = tempfile::tempdir().unwrap();
    let mut results = Vec::new();

    // Small package: 20 modules, 2 entries.
    let small_root = temp.path().join("small");
    generate_package(&small_root, 20, 2);

    let small_entries: Vec<PathBuf> = (0..2)
        .map(|e| PathBuf::from(format!("src/entry{e}.ts")))
        .collect();

    results.push(bench("small (20 modules, 2 entries)", 100, || {
        compile(CompileOptions {
            package_root: small_root.clone(),
            entries: small_entries.clone(),
            source_maps: false,
        })
        .unwrap();
    }));

    // Medium package: 100 modules, 5 entries.
    let medium_root = temp.path().join("medium");
    generate_package(&medium_root, 100, 5);

    let medium_entries: Vec<PathBuf> = (0..5)
        .map(|e| PathBuf::from(format!("src/entry{e}.ts")))
        .collect();

    results.push(bench("medium (100 modules, 5 entries)", 20, || {
        compile(CompileOptions {
            package_root: medium_root.clone(),
            entries: medium_entries.clone(),
            source_maps: false,
        })
        .unwrap();
    }));

    // Large package: 500 modules, 10 entries.
    let large_root = temp.path().join("large");
    generate_package(&large_root, 500, 10);

    let large_entries: Vec<PathBuf> = (0..10)
        .map(|e| PathBuf::from(format!("src/entry{e}.ts")))
        .collect();

    results.push(bench("large (500 modules, 10 entries)", 5, || {
        compile(CompileOptions {
            package_root: large_root.clone(),
            entries: large_entries.clone(),
            source_maps: false,
        })
        .unwrap();
    }));

    // Write machine-readable JSON for CI (github-action-benchmark customSmallerIsBetter format).
    let json: Vec<String> = results
        .iter()
        .map(|r| {
            format!(
                r#"  {{ "name": "{}", "unit": "ns/iter", "value": {} }}"#,
                r.name, r.value_ns
            )
        })
        .collect();
    let json_output = format!("[\n{}\n]", json.join(",\n"));
    let output_path =
        std::env::var("BENCH_OUTPUT").unwrap_or_else(|_| "bench-results.json".to_string());
    std::fs::write(output_path, json_output).ok();
}
