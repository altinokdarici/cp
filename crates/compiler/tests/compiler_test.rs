use std::path::PathBuf;

use compiler::{CompileOptions, compile};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

#[test]
fn test_single_entry_simple_package() {
    let package_root = fixtures_dir().join("simple-pkg");

    let result = compile(CompileOptions {
        package_root: package_root.clone(),
        entries: vec![PathBuf::from("src/index.ts")],
    })
    .expect("Compilation should succeed");

    // Should produce one output file (single entry, no shared chunks needed).
    assert!(
        !result.files.is_empty(),
        "Should produce at least one output file"
    );

    // Find the entry output.
    let entry_file = result
        .files
        .iter()
        .find(|f| f.name.contains("index"))
        .unwrap();

    // Should contain the transformed JS (no TypeScript types).
    assert!(
        !entry_file.content.contains(": string"),
        "TypeScript types should be stripped"
    );

    // Should contain the function bodies.
    assert!(
        entry_file.content.contains("Hello"),
        "Should contain string literal from greet.ts"
    );
    assert!(
        entry_file.content.contains("world"),
        "Should contain string literal from helpers.ts"
    );

    // No external dependencies in this package.
    assert!(
        result.manifest.externals.is_empty(),
        "Simple package has no external deps, got: {:?}",
        result.manifest.externals
    );

    println!("=== Single Entry Output ===");
    for file in &result.files {
        println!("--- {} ---", file.name);
        println!("{}", file.content);
    }
}

#[test]
fn test_multi_entry_with_shared_chunks() {
    let package_root = fixtures_dir().join("multi-entry-pkg");

    let result = compile(CompileOptions {
        package_root: package_root.clone(),
        entries: vec![
            PathBuf::from("src/index.ts"),
            PathBuf::from("src/Button.ts"),
        ],
    })
    .expect("Compilation should succeed");

    // Should have external dependency on 'react'.
    assert!(
        result.manifest.externals.contains(&"react".to_string()),
        "Should detect 'react' as external, got: {:?}",
        result.manifest.externals
    );

    // utils.ts is imported by both entries → should become a shared chunk.
    let chunk_file = result.files.iter().find(|f| f.name.contains("chunk"));
    assert!(
        chunk_file.is_some(),
        "Should produce a shared chunk for utils.ts, files: {:?}",
        result.files.iter().map(|f| &f.name).collect::<Vec<_>>()
    );

    // No TypeScript types in any output.
    for file in &result.files {
        assert!(
            !file.content.contains("interface ButtonProps"),
            "TypeScript interfaces should be stripped in {}",
            file.name
        );
        assert!(
            !file.content.contains(": string"),
            "TypeScript type annotations should be stripped in {}",
            file.name
        );
    }

    println!("=== Multi Entry Output ===");
    for file in &result.files {
        println!("--- {} ---", file.name);
        println!("{}", file.content);
    }
}

#[test]
fn test_export_reexports_are_traced() {
    let package_root = fixtures_dir().join("reexport-pkg");

    let result = compile(CompileOptions {
        package_root,
        entries: vec![PathBuf::from("src/index.ts")],
    })
    .expect("Compilation should succeed");

    let entry = &result.files[0];

    // export * from './utils' should pull in utils/index.ts content.
    assert!(
        entry.content.contains("toUpperCase"),
        "Should contain content from utils via export * from, got:\n{}",
        entry.content
    );
    assert!(
        entry.content.contains("trim"),
        "Should contain trimName from utils via export * from"
    );

    // export { greet } from './greet' should pull in greet.ts content.
    assert!(
        entry.content.contains("Hello"),
        "Should contain content from greet.ts via named re-export"
    );

    // Both 'lodash' (from index.ts) and 'react' (from greet.ts) should be externals.
    assert!(
        result.manifest.externals.contains(&"lodash".to_string()),
        "Should detect 'lodash' as external, got: {:?}",
        result.manifest.externals
    );
    assert!(
        result.manifest.externals.contains(&"react".to_string()),
        "Should detect 'react' as external from re-exported module, got: {:?}",
        result.manifest.externals
    );
}

#[test]
fn test_multi_index_entry_naming() {
    let package_root = fixtures_dir().join("multi-index-pkg");

    let result = compile(CompileOptions {
        package_root,
        entries: vec![
            PathBuf::from("src/appChrome/index.ts"),
            PathBuf::from("src/header/index.ts"),
        ],
    })
    .expect("Compilation should succeed");

    let entry_names: Vec<&str> = result.files.iter().map(|f| f.name.as_str()).collect();

    // Entries should be named after parent directories, not both "index.js".
    assert!(
        entry_names.contains(&"appChrome.js"),
        "Should name entry after parent dir 'appChrome', got: {:?}",
        entry_names
    );
    assert!(
        entry_names.contains(&"header.js"),
        "Should name entry after parent dir 'header', got: {:?}",
        entry_names
    );
    assert!(
        !entry_names.iter().filter(|n| **n == "index.js").count() > 1,
        "Should not have duplicate index.js entries"
    );

    // Both entries share helper.ts, so there should be a shared chunk.
    assert!(
        result.manifest.chunks.iter().any(|c| c.contains("chunk")),
        "Should produce a shared chunk for helper.ts, got: {:?}",
        result.manifest.chunks
    );
}

#[test]
fn test_entry_name_dedup_on_collision() {
    // Two entries that would both resolve to the same parent name get deduplicated.
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    std::fs::write(
        root.join("package.json"),
        r#"{"name": "dedup-pkg", "version": "1.0.0"}"#,
    )
    .unwrap();

    // Two directories with the same name at different depths.
    std::fs::create_dir_all(root.join("src/feature")).unwrap();
    std::fs::create_dir_all(root.join("src/v2/feature")).unwrap();

    std::fs::write(
        root.join("src/feature/index.ts"),
        "export const a: string = 'v1';",
    )
    .unwrap();

    std::fs::write(
        root.join("src/v2/feature/index.ts"),
        "export const b: string = 'v2';",
    )
    .unwrap();

    let result = compile(CompileOptions {
        package_root: root.to_path_buf(),
        entries: vec![
            PathBuf::from("src/feature/index.ts"),
            PathBuf::from("src/v2/feature/index.ts"),
        ],
    })
    .expect("Compilation should succeed");

    let entry_names: Vec<&str> = result.files.iter().map(|f| f.name.as_str()).collect();

    // Should produce two distinct output files, not overwrite.
    assert_eq!(
        result.files.len(),
        2,
        "Should produce two output files, got: {:?}",
        entry_names
    );
    assert_ne!(
        entry_names[0], entry_names[1],
        "Entry names must be unique, got: {:?}",
        entry_names
    );

    // First gets "feature.js", second gets "feature-1.js".
    assert!(
        entry_names.contains(&"feature.js"),
        "First entry should be feature.js, got: {:?}",
        entry_names
    );
    assert!(
        entry_names.contains(&"feature-1.js"),
        "Second entry should be feature-1.js, got: {:?}",
        entry_names
    );
}

#[test]
fn test_circular_dependency_succeeds() {
    // Circular imports are valid in JS/TS and should compile successfully.
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    std::fs::write(
        root.join("package.json"),
        r#"{"name": "circular-pkg", "version": "1.0.0"}"#,
    )
    .unwrap();

    std::fs::create_dir_all(root.join("src")).unwrap();

    std::fs::write(
        root.join("src/a.ts"),
        r#"import { b } from './b';
export const a: string = 'a' + b;"#,
    )
    .unwrap();

    std::fs::write(
        root.join("src/b.ts"),
        r#"import { a } from './a';
export const b: string = 'b' + a;"#,
    )
    .unwrap();

    let result = compile(CompileOptions {
        package_root: root.to_path_buf(),
        entries: vec![PathBuf::from("src/a.ts")],
    });

    assert!(
        result.is_ok(),
        "Circular dependencies should compile successfully"
    );

    let output = result.unwrap();
    assert_eq!(output.files.len(), 1, "Should produce one output file");
    // Both modules should be included in the output.
    let content = &output.files[0].content;
    assert!(
        content.contains("a") && content.contains("b"),
        "Should contain content from both modules, got: {}",
        content
    );
}
