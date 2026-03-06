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
fn test_circular_dependency_fails() {
    // Create a temp directory with circular imports.
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

    assert!(result.is_err(), "Should fail on circular dependency");
    assert!(
        result.unwrap_err().contains("Circular"),
        "Error should mention circular dependency"
    );
}
