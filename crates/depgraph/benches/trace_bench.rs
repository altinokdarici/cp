use std::path::PathBuf;
use std::time::Instant;

use depgraph::{TraceOptions, trace};

struct BenchResult {
    name: String,
    value_ns: u128,
}

/// Generate a synthetic app with N internal modules and M npm packages.
/// Each package has `modules_per_pkg` internal modules and imports from
/// a chain of other packages to create transitive dependency depth.
fn generate_app(
    root: &std::path::Path,
    num_app_modules: usize,
    num_packages: usize,
    modules_per_pkg: usize,
) {
    let src = root.join("src");
    std::fs::create_dir_all(&src).unwrap();
    let nm = root.join("node_modules");

    // App package.json.
    std::fs::write(
        root.join("package.json"),
        r#"{"name": "bench-app", "version": "1.0.0"}"#,
    )
    .unwrap();

    // App entry — imports internal modules + all direct dep packages.
    let mut entry = String::new();
    for i in 0..num_app_modules {
        entry.push_str(&format!("import {{ mod{i} }} from './mod{i}';\n"));
    }
    // Import the first half of packages directly (the rest are transitive).
    let direct_deps = (num_packages / 2).max(1);
    for i in 0..direct_deps {
        entry.push_str(&format!("import {{ pkg{i} }} from 'pkg-{i}';\n"));
    }
    entry.push_str("export const app = 'bench';\n");
    std::fs::write(src.join("index.ts"), entry).unwrap();

    // App internal modules — each imports a few neighbors.
    for i in 0..num_app_modules {
        let mut source = String::new();
        if i > 0 {
            source.push_str(&format!(
                "import {{ mod{} }} from './mod{}';\n",
                i - 1,
                i - 1
            ));
        }
        if i > 2 {
            source.push_str(&format!(
                "import {{ mod{} }} from './mod{}';\n",
                i - 2,
                i - 2
            ));
        }
        source.push_str(&format!(
            "export function mod{i}(): string {{ return 'mod{i}'; }}\n"
        ));
        std::fs::write(src.join(format!("mod{i}.ts")), source).unwrap();
    }

    // Generate packages in a chain: pkg-0 → pkg-1 → pkg-2 → ...
    for i in 0..num_packages {
        let pkg_name = format!("pkg-{i}");
        let pkg_dir = nm.join(&pkg_name);
        std::fs::create_dir_all(&pkg_dir).unwrap();

        std::fs::write(
            pkg_dir.join("package.json"),
            format!(r#"{{"name": "{pkg_name}", "version": "1.0.0", "main": "index.js"}}"#),
        )
        .unwrap();

        // Index — imports internal modules + next package in chain (transitive dep).
        let mut index = String::new();
        for j in 0..modules_per_pkg {
            index.push_str(&format!("import {{ helper{j} }} from './helper{j}';\n"));
        }
        if i + 1 < num_packages {
            index.push_str(&format!(
                "import {{ pkg{} }} from 'pkg-{}';\n",
                i + 1,
                i + 1
            ));
        }
        index.push_str(&format!(
            "export function pkg{i}() {{ return 'pkg-{i}'; }}\n"
        ));
        std::fs::write(pkg_dir.join("index.js"), index).unwrap();

        // Internal helper modules.
        for j in 0..modules_per_pkg {
            let mut helper = String::new();
            if j > 0 {
                helper.push_str(&format!(
                    "import {{ helper{} }} from './helper{}';\n",
                    j - 1,
                    j - 1
                ));
            }
            helper.push_str(&format!(
                "export function helper{j}() {{ return 'helper{j}'; }}\n"
            ));
            std::fs::write(pkg_dir.join(format!("helper{j}.js")), helper).unwrap();
        }
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

    // Small: 10 app modules, 5 packages × 3 helpers = ~30 total files.
    let small_root = temp.path().join("small");
    generate_app(&small_root, 10, 5, 3);

    results.push(bench("small (10 modules, 5 packages)", 100, || {
        trace(TraceOptions {
            app_root: small_root.clone(),
            entries: vec![PathBuf::from("src/index.ts")],
        })
        .unwrap();
    }));

    // Medium: 50 app modules, 20 packages × 5 helpers = ~170 total files.
    let medium_root = temp.path().join("medium");
    generate_app(&medium_root, 50, 20, 5);

    results.push(bench("medium (50 modules, 20 packages)", 20, || {
        trace(TraceOptions {
            app_root: medium_root.clone(),
            entries: vec![PathBuf::from("src/index.ts")],
        })
        .unwrap();
    }));

    // Large: 100 app modules, 50 packages × 10 helpers = ~650 total files.
    let large_root = temp.path().join("large");
    generate_app(&large_root, 100, 50, 10);

    results.push(bench("large (100 modules, 50 packages)", 5, || {
        trace(TraceOptions {
            app_root: large_root.clone(),
            entries: vec![PathBuf::from("src/index.ts")],
        })
        .unwrap();
    }));

    // Write machine-readable JSON.
    let json: Vec<String> = results
        .iter()
        .map(|r| {
            format!(
                r#"  {{ "name": "depgraph::{}", "unit": "ns/iter", "value": {} }}"#,
                r.name, r.value_ns
            )
        })
        .collect();
    let json_output = format!("[\n{}\n]", json.join(",\n"));
    let output_path =
        std::env::var("BENCH_OUTPUT").unwrap_or_else(|_| "bench-results.json".to_string());
    std::fs::write(output_path, json_output).ok();
}
