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
        source_maps: false,
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
        source_maps: false,
    })
    .expect("Compilation should succeed");

    // Should have external dependency on 'react'.
    assert!(
        result.manifest.externals.contains(&"react".to_string()),
        "Should detect 'react' as external, got: {:?}",
        result.manifest.externals
    );

    // utils.ts is imported by both entries → should become a shared chunk.
    let chunk_file = result.files.iter().find(|f| f.name == "chunk-0-1.js");
    assert!(
        chunk_file.is_some(),
        "Should produce chunk-0-1.js for utils.ts, files: {:?}",
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
        source_maps: false,
    })
    .expect("Compilation should succeed");

    let entry = result
        .files
        .iter()
        .find(|f| f.name.contains("index"))
        .expect("Should have an index entry file");

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
        source_maps: false,
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
        entry_names.iter().filter(|n| **n == "index.js").count() <= 1,
        "Should not have duplicate index.js entries"
    );

    // Both entries share helper.ts, so there should be a shared chunk.
    assert!(
        result.manifest.chunks.iter().any(|c| c == "chunk-0-1.js"),
        "Should produce chunk-0-1.js for helper.ts, got: {:?}",
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
        source_maps: false,
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
        source_maps: false,
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
        content.contains("const a ="),
        "Should contain a.ts declaration, got: {}",
        content
    );
    assert!(
        content.contains("const b ="),
        "Should contain b.ts declaration, got: {}",
        content
    );
}

#[test]
fn test_multiline_imports_are_stripped() {
    // Multi-line imports should be correctly stripped at the AST level.
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    std::fs::write(
        root.join("package.json"),
        r#"{"name": "multiline-pkg", "version": "1.0.0"}"#,
    )
    .unwrap();

    std::fs::create_dir_all(root.join("src")).unwrap();

    std::fs::write(
        root.join("src/helpers.ts"),
        "export const foo: string = 'foo';\nexport const bar: string = 'bar';\n",
    )
    .unwrap();

    // Entry with a multi-line import and multi-line re-export.
    std::fs::write(
        root.join("src/index.ts"),
        r#"import {
  foo,
  bar
} from './helpers';

export {
  foo,
  bar
} from './helpers';

import {
  createElement
} from 'react';

export const result: string = foo + bar;
"#,
    )
    .unwrap();

    let result = compile(CompileOptions {
        package_root: root.to_path_buf(),
        entries: vec![PathBuf::from("src/index.ts")],
        source_maps: false,
    })
    .expect("Compilation should succeed");

    let entry = &result.files[0];

    // The multi-line internal import should be fully stripped.
    assert!(
        !entry.content.contains("from './helpers'"),
        "Internal imports should be stripped, got:\n{}",
        entry.content
    );
    assert!(
        !entry.content.contains("from \"./helpers\""),
        "Internal imports should be stripped (double quotes), got:\n{}",
        entry.content
    );

    // The multi-line external import should also be stripped (linker re-adds externals).
    assert!(
        !entry.content.contains("createElement"),
        "Original external import should be stripped (linker re-adds), got:\n{}",
        entry.content
    );

    // The function body should still be present.
    assert!(
        entry.content.contains("result"),
        "Should contain the result export, got:\n{}",
        entry.content
    );

    // Content from helpers should be inlined.
    assert!(
        entry.content.contains("foo"),
        "Should contain helpers content, got:\n{}",
        entry.content
    );

    // react should be detected as external.
    assert!(
        result.manifest.externals.contains(&"react".to_string()),
        "Should detect 'react' as external, got: {:?}",
        result.manifest.externals
    );
}

#[test]
fn test_source_maps_generated() {
    let package_root = fixtures_dir().join("simple-pkg");

    let result = compile(CompileOptions {
        package_root,
        entries: vec![PathBuf::from("src/index.ts")],
        source_maps: true,
    })
    .expect("Compilation should succeed");

    // At least one output file should have a source map.
    let entry = &result.files[0];
    assert!(
        entry.source_map.is_some(),
        "Entry file should have a source map when source_maps is enabled"
    );

    // Source map should be valid JSON with a "sources" array.
    let map_json: serde_json::Value = serde_json::from_str(entry.source_map.as_ref().unwrap())
        .expect("Source map should be valid JSON");
    assert!(
        map_json.get("sources").is_some(),
        "Source map should have a 'sources' field, got: {}",
        map_json
    );
    assert!(map_json["sources"].is_array(), "sources should be an array");

    // Content should have a sourceMappingURL comment.
    assert!(
        entry.content.contains("//# sourceMappingURL="),
        "Output should contain sourceMappingURL comment, got:\n{}",
        entry.content
    );
}

#[test]
fn test_source_maps_disabled_by_default() {
    let package_root = fixtures_dir().join("simple-pkg");

    let result = compile(CompileOptions {
        package_root,
        entries: vec![PathBuf::from("src/index.ts")],
        source_maps: false,
    })
    .expect("Compilation should succeed");

    for file in &result.files {
        assert!(
            file.source_map.is_none(),
            "File {} should not have a source map when source_maps is false",
            file.name
        );
        assert!(
            !file.content.contains("//# sourceMappingURL="),
            "File {} should not contain sourceMappingURL when source_maps is false",
            file.name
        );
    }
}

#[test]
fn test_multiple_shared_chunks() {
    // 3 entries where different pairs share different modules → separate chunks.
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();

    std::fs::write(
        root.join("package.json"),
        r#"{"name": "multi-chunk-pkg", "version": "1.0.0"}"#,
    )
    .unwrap();

    std::fs::create_dir_all(root.join("src")).unwrap();

    // shared_ab.ts — shared by entry A and B only.
    std::fs::write(
        root.join("src/shared_ab.ts"),
        "export const sharedAB: string = 'ab';",
    )
    .unwrap();

    // shared_bc.ts — shared by entry B and C only.
    std::fs::write(
        root.join("src/shared_bc.ts"),
        "export const sharedBC: string = 'bc';",
    )
    .unwrap();

    // shared_all.ts — shared by all three entries.
    std::fs::write(
        root.join("src/shared_all.ts"),
        "export const sharedAll: string = 'all';",
    )
    .unwrap();

    // Entry A imports shared_ab and shared_all.
    std::fs::write(
        root.join("src/entryA.ts"),
        r#"import { sharedAB } from './shared_ab';
import { sharedAll } from './shared_all';
export const a: string = sharedAB + sharedAll;"#,
    )
    .unwrap();

    // Entry B imports shared_ab, shared_bc, and shared_all.
    std::fs::write(
        root.join("src/entryB.ts"),
        r#"import { sharedAB } from './shared_ab';
import { sharedBC } from './shared_bc';
import { sharedAll } from './shared_all';
export const b: string = sharedAB + sharedBC + sharedAll;"#,
    )
    .unwrap();

    // Entry C imports shared_bc and shared_all.
    std::fs::write(
        root.join("src/entryC.ts"),
        r#"import { sharedBC } from './shared_bc';
import { sharedAll } from './shared_all';
export const c: string = sharedBC + sharedAll;"#,
    )
    .unwrap();

    let result = compile(CompileOptions {
        package_root: root.to_path_buf(),
        entries: vec![
            PathBuf::from("src/entryA.ts"),
            PathBuf::from("src/entryB.ts"),
            PathBuf::from("src/entryC.ts"),
        ],
        source_maps: false,
    })
    .expect("Compilation should succeed");

    let chunk_names: Vec<&str> = result.manifest.chunks.iter().map(|s| s.as_str()).collect();

    // Should have 3 separate shared chunks:
    // chunk-0-1.js (shared by entries 0,1 = entryA, entryB) for shared_ab
    // chunk-1-2.js (shared by entries 1,2 = entryB, entryC) for shared_bc
    // chunk-0-1-2.js (shared by all three) for shared_all
    assert!(
        chunk_names.contains(&"chunk-0-1.js"),
        "Should have chunk-0-1.js for shared_ab, got: {:?}",
        chunk_names
    );
    assert!(
        chunk_names.contains(&"chunk-1-2.js"),
        "Should have chunk-1-2.js for shared_bc, got: {:?}",
        chunk_names
    );
    assert!(
        chunk_names.contains(&"chunk-0-1-2.js"),
        "Should have chunk-0-1-2.js for shared_all, got: {:?}",
        chunk_names
    );

    // Each entry should only import the chunks it belongs to.
    let entry_a = result.files.iter().find(|f| f.name == "entryA.js").unwrap();
    let entry_b = result.files.iter().find(|f| f.name == "entryB.js").unwrap();
    let entry_c = result.files.iter().find(|f| f.name == "entryC.js").unwrap();

    // Entry A should import chunk-0-1 and chunk-0-1-2, but NOT chunk-1-2.
    assert!(
        entry_a.content.contains("chunk-0-1.js"),
        "Entry A should import chunk-0-1.js"
    );
    assert!(
        entry_a.content.contains("chunk-0-1-2.js"),
        "Entry A should import chunk-0-1-2.js"
    );
    assert!(
        !entry_a.content.contains("chunk-1-2.js"),
        "Entry A should NOT import chunk-1-2.js"
    );

    // Entry B should import all three chunks.
    assert!(
        entry_b.content.contains("chunk-0-1.js"),
        "Entry B should import chunk-0-1.js"
    );
    assert!(
        entry_b.content.contains("chunk-1-2.js"),
        "Entry B should import chunk-1-2.js"
    );
    assert!(
        entry_b.content.contains("chunk-0-1-2.js"),
        "Entry B should import chunk-0-1-2.js"
    );

    // Entry C should import chunk-1-2 and chunk-0-1-2, but NOT chunk-0-1.
    assert!(
        entry_c.content.contains("chunk-1-2.js"),
        "Entry C should import chunk-1-2.js"
    );
    assert!(
        entry_c.content.contains("chunk-0-1-2.js"),
        "Entry C should import chunk-0-1-2.js"
    );
    assert!(
        !entry_c.content.contains("chunk-0-1.js"),
        "Entry C should NOT import chunk-0-1.js"
    );
}
