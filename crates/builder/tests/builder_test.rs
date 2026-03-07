use std::path::PathBuf;

use builder::{BuildOptions, build};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures")
}

#[test]
fn test_single_dep() {
    let app_root = fixtures_dir().join("simple-app");

    let output = build(BuildOptions {
        app_root: app_root.clone(),
        entries: vec![PathBuf::from("src/index.ts")],
        source_maps: false,
    })
    .expect("Build should succeed");

    // App should be compiled.
    assert!(!output.app.files.is_empty(), "App should have output files");
    assert_eq!(output.app.name, "_app");
    assert_eq!(output.app.virtual_prefix, "/@cp/_app/");

    // react should be in the import map.
    assert!(
        output.import_map.imports.contains_key("react"),
        "Import map should contain 'react', got: {:?}",
        output.import_map.imports
    );

    // The react entry should point to the virtual path.
    let react_path = &output.import_map.imports["react"];
    assert!(
        react_path.starts_with("/@cp/react@18.0.0/"),
        "react virtual path should start with /@cp/react@18.0.0/, got: {}",
        react_path
    );
}

#[test]
fn test_transitive_deps() {
    let app_root = fixtures_dir().join("simple-app");

    let output = build(BuildOptions {
        app_root: app_root.clone(),
        entries: vec![PathBuf::from("src/index.ts")],
        source_maps: false,
    })
    .expect("Build should succeed");

    // scheduler is imported by react (transitive) → should be in the import map.
    assert!(
        output.import_map.imports.contains_key("scheduler"),
        "Import map should contain transitive dep 'scheduler', got: {:?}",
        output.import_map.imports
    );

    let scheduler_path = &output.import_map.imports["scheduler"];
    assert!(
        scheduler_path.starts_with("/@cp/scheduler@0.23.0/"),
        "scheduler virtual path should start with /@cp/scheduler@0.23.0/, got: {}",
        scheduler_path
    );
}

#[test]
fn test_subpath_imports() {
    let app_root = fixtures_dir().join("subpath-app");

    let output = build(BuildOptions {
        app_root: app_root.clone(),
        entries: vec![PathBuf::from("src/index.ts")],
        source_maps: false,
    })
    .expect("Build should succeed");

    // Both react and react/jsx-runtime should be in the import map.
    assert!(
        output.import_map.imports.contains_key("react"),
        "Import map should contain 'react', got: {:?}",
        output.import_map.imports
    );
    assert!(
        output.import_map.imports.contains_key("react/jsx-runtime"),
        "Import map should contain 'react/jsx-runtime', got: {:?}",
        output.import_map.imports
    );

    // Both should share the same react@ prefix.
    let react_path = &output.import_map.imports["react"];
    let jsx_path = &output.import_map.imports["react/jsx-runtime"];
    assert!(
        react_path.starts_with("/@cp/react@18.0.0/"),
        "react path: {}",
        react_path
    );
    assert!(
        jsx_path.starts_with("/@cp/react@18.0.0/"),
        "jsx-runtime path: {}",
        jsx_path
    );

    // They should be compiled as part of the same package (single react entry in packages).
    let react_pkgs: Vec<_> = output
        .packages
        .iter()
        .filter(|p| p.name == "react")
        .collect();
    assert_eq!(
        react_pkgs.len(),
        1,
        "react should be compiled once, got {} packages",
        react_pkgs.len()
    );
}

#[test]
fn test_scoped_packages() {
    let app_root = fixtures_dir().join("scoped-app");

    let output = build(BuildOptions {
        app_root: app_root.clone(),
        entries: vec![PathBuf::from("src/index.ts")],
        source_maps: false,
    })
    .expect("Build should succeed");

    assert!(
        output.import_map.imports.contains_key("@scope/pkg"),
        "Import map should contain '@scope/pkg', got: {:?}",
        output.import_map.imports
    );

    let path = &output.import_map.imports["@scope/pkg"];
    assert!(
        path.starts_with("/@cp/@scope/pkg@2.1.0/"),
        "Scoped package virtual path should contain /@cp/@scope/pkg@2.1.0/, got: {}",
        path
    );
}

#[test]
fn test_import_map_correctness() {
    let app_root = fixtures_dir().join("simple-app");

    let output = build(BuildOptions {
        app_root: app_root.clone(),
        entries: vec![PathBuf::from("src/index.ts")],
        source_maps: false,
    })
    .expect("Build should succeed");

    // All import map values should start with /@cp/.
    for (specifier, path) in &output.import_map.imports {
        assert!(
            path.starts_with("/@cp/"),
            "Import map value for '{}' should start with /@cp/, got: {}",
            specifier,
            path
        );
    }

    // All import map values should contain a version (@ followed by digits).
    for (specifier, path) in &output.import_map.imports {
        assert!(
            path.contains('@'),
            "Import map value for '{}' should contain version (@), got: {}",
            specifier,
            path
        );
    }

    // Import map should be serializable to JSON.
    let json = serde_json::to_string_pretty(&output.import_map).unwrap();
    assert!(json.contains("\"imports\""), "JSON should have imports key");
}

#[test]
fn test_app_compilation() {
    let app_root = fixtures_dir().join("simple-app");

    let output = build(BuildOptions {
        app_root: app_root.clone(),
        entries: vec![PathBuf::from("src/index.ts")],
        source_maps: false,
    })
    .expect("Build should succeed");

    // App should have compiled files.
    assert!(!output.app.files.is_empty(), "App should produce files");

    // App entry file should contain the app code (TypeScript stripped).
    let entry = &output
        .app
        .files
        .iter()
        .find(|f| f.name.contains("index"))
        .unwrap();
    assert!(
        !entry.content.contains(": string"),
        "TypeScript types should be stripped from app code"
    );
    // The app should contain inlined content from utils.ts.
    assert!(
        entry.content.contains("Hello"),
        "App code should contain string literal from utils, got:\n{}",
        entry.content
    );
}
