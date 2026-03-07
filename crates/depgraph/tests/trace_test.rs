use std::path::PathBuf;

use depgraph::{TraceOptions, trace};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("tests/fixtures")
}

// ── Small fixtures ──────────────────────────────────────────────────

#[test]
fn test_simple_app_externals() {
    let output = trace(TraceOptions {
        app_root: fixtures_dir().join("simple-app"),
        entries: vec![PathBuf::from("src/index.ts")],
    })
    .expect("Trace should succeed");

    let react = output
        .packages
        .iter()
        .find(|p| p.name == "react")
        .expect("react should be discovered");
    assert_eq!(react.version, "18.0.0");
    assert!(react.specifiers.contains(&"react".to_string()));
}

#[test]
fn test_transitive_discovery() {
    let output = trace(TraceOptions {
        app_root: fixtures_dir().join("simple-app"),
        entries: vec![PathBuf::from("src/index.ts")],
    })
    .expect("Trace should succeed");

    let scheduler = output
        .packages
        .iter()
        .find(|p| p.name == "scheduler")
        .expect("scheduler should be transitively discovered");
    assert_eq!(scheduler.version, "0.23.0");
}

#[test]
fn test_subpath_entries() {
    let output = trace(TraceOptions {
        app_root: fixtures_dir().join("subpath-app"),
        entries: vec![PathBuf::from("src/index.ts")],
    })
    .expect("Trace should succeed");

    let react = output
        .packages
        .iter()
        .find(|p| p.name == "react")
        .expect("react should be discovered");

    assert_eq!(react.entries.len(), 2, "react entries: {:?}", react.entries);
    assert_eq!(
        react.specifiers.len(),
        2,
        "react specifiers: {:?}",
        react.specifiers
    );
    assert!(react.specifiers.contains(&"react".to_string()));
    assert!(react.specifiers.contains(&"react/jsx-runtime".to_string()));

    for entry in &react.entries {
        assert!(
            entry.is_relative(),
            "Entry should be relative: {}",
            entry.display()
        );
    }
}

#[test]
fn test_scoped_packages() {
    let output = trace(TraceOptions {
        app_root: fixtures_dir().join("scoped-app"),
        entries: vec![PathBuf::from("src/index.ts")],
    })
    .expect("Trace should succeed");

    let pkg = output
        .packages
        .iter()
        .find(|p| p.name == "@scope/pkg")
        .expect("@scope/pkg should be discovered");
    assert_eq!(pkg.version, "2.1.0");
}

#[test]
fn test_entries_are_relative() {
    let output = trace(TraceOptions {
        app_root: fixtures_dir().join("simple-app"),
        entries: vec![PathBuf::from("src/index.ts")],
    })
    .expect("Trace should succeed");

    for pkg in &output.packages {
        for entry in &pkg.entries {
            assert!(
                entry.is_relative(),
                "Entry {} in {} should be relative",
                entry.display(),
                pkg.name
            );
        }
    }
}

#[test]
fn test_no_duplicate_packages() {
    let output = trace(TraceOptions {
        app_root: fixtures_dir().join("simple-app"),
        entries: vec![PathBuf::from("src/index.ts")],
    })
    .expect("Trace should succeed");

    let mut seen = std::collections::HashSet::new();
    for pkg in &output.packages {
        let key = format!("{}@{}", pkg.name, pkg.version);
        assert!(seen.insert(key.clone()), "Duplicate package: {key}");
    }
}

#[test]
fn test_specifier_entry_alignment() {
    let output = trace(TraceOptions {
        app_root: fixtures_dir().join("subpath-app"),
        entries: vec![PathBuf::from("src/index.ts")],
    })
    .expect("Trace should succeed");

    for pkg in &output.packages {
        assert_eq!(
            pkg.specifiers.len(),
            pkg.entries.len(),
            "Package {} has {} specifiers but {} entries",
            pkg.name,
            pkg.specifiers.len(),
            pkg.entries.len()
        );
    }
}

// ── Medium fixture ──────────────────────────────────────────────────

#[test]
fn test_medium_app_diamond_deps() {
    let output = trace(TraceOptions {
        app_root: fixtures_dir().join("medium-app"),
        entries: vec![PathBuf::from("src/index.ts")],
    })
    .expect("Trace should succeed");

    // Direct deps: lodash (subpath import), @viz/chart (via dashboard/chart).
    let lodash = output.packages.iter().find(|p| p.name == "lodash");
    assert!(lodash.is_some(), "lodash should be discovered");

    let viz_chart = output.packages.iter().find(|p| p.name == "@viz/chart");
    assert!(viz_chart.is_some(), "@viz/chart should be discovered");

    // Diamond dep: event-emitter is imported by both @viz/chart and color-utils.
    let event_emitter = output.packages.iter().find(|p| p.name == "event-emitter");
    assert!(
        event_emitter.is_some(),
        "event-emitter should be discovered (diamond dep)"
    );

    // event-emitter should appear exactly once despite being reached via two paths.
    let ee_count = output
        .packages
        .iter()
        .filter(|p| p.name == "event-emitter")
        .count();
    assert_eq!(ee_count, 1, "event-emitter should appear exactly once");
}

#[test]
fn test_medium_app_scoped_and_subpath() {
    let output = trace(TraceOptions {
        app_root: fixtures_dir().join("medium-app"),
        entries: vec![PathBuf::from("src/index.ts")],
    })
    .expect("Trace should succeed");

    // Scoped package.
    assert!(
        output.packages.iter().any(|p| p.name == "@viz/chart"),
        "@viz/chart should be found"
    );

    // Subpath import: lodash/merge should be a specifier for lodash.
    let lodash = output
        .packages
        .iter()
        .find(|p| p.name == "lodash")
        .expect("lodash should be found");
    assert!(
        lodash.specifiers.iter().any(|s| s == "lodash/merge"),
        "lodash should have 'lodash/merge' specifier, got: {:?}",
        lodash.specifiers
    );
}

#[test]
fn test_medium_app_total_packages() {
    let output = trace(TraceOptions {
        app_root: fixtures_dir().join("medium-app"),
        entries: vec![PathBuf::from("src/index.ts")],
    })
    .expect("Trace should succeed");

    // Should discover: @viz/chart, lodash, event-emitter, color-utils = 4 packages.
    let names: Vec<&str> = output.packages.iter().map(|p| p.name.as_str()).collect();
    assert!(
        names.contains(&"@viz/chart"),
        "missing @viz/chart: {names:?}"
    );
    assert!(names.contains(&"lodash"), "missing lodash: {names:?}");
    assert!(
        names.contains(&"event-emitter"),
        "missing event-emitter: {names:?}"
    );
    assert!(
        names.contains(&"color-utils"),
        "missing color-utils: {names:?}"
    );
    assert_eq!(
        output.packages.len(),
        4,
        "Expected 4 packages, got: {names:?}"
    );
}

// ── Large fixture ───────────────────────────────────────────────────

#[test]
fn test_large_app_multi_entry() {
    let output = trace(TraceOptions {
        app_root: fixtures_dir().join("large-app"),
        entries: vec![PathBuf::from("src/index.ts"), PathBuf::from("src/admin.ts")],
    })
    .expect("Trace should succeed");

    // Both entries should discover react.
    assert!(
        output.packages.iter().any(|p| p.name == "react"),
        "react should be discovered"
    );
}

#[test]
fn test_large_app_deep_chain() {
    let output = trace(TraceOptions {
        app_root: fixtures_dir().join("large-app"),
        entries: vec![PathBuf::from("src/index.ts")],
    })
    .expect("Trace should succeed");

    // Deep chain: app → @http/client → @http/headers
    assert!(
        output.packages.iter().any(|p| p.name == "@http/client"),
        "@http/client should be discovered"
    );
    assert!(
        output.packages.iter().any(|p| p.name == "@http/headers"),
        "@http/headers should be discovered (transitive via @http/client)"
    );
}

#[test]
fn test_large_app_diamond_deps() {
    let output = trace(TraceOptions {
        app_root: fixtures_dir().join("large-app"),
        entries: vec![PathBuf::from("src/index.ts")],
    })
    .expect("Trace should succeed");

    // Diamond: react → scheduler, react-internals → scheduler
    let scheduler_count = output
        .packages
        .iter()
        .filter(|p| p.name == "scheduler")
        .count();
    assert_eq!(
        scheduler_count, 1,
        "scheduler should appear exactly once despite diamond dep"
    );
}

#[test]
fn test_large_app_package_count() {
    let output = trace(TraceOptions {
        app_root: fixtures_dir().join("large-app"),
        entries: vec![PathBuf::from("src/index.ts"), PathBuf::from("src/admin.ts")],
    })
    .expect("Trace should succeed");

    // 10 packages: react, scheduler, react-internals, @ui/components, @ui/theme,
    //              @store/state, event-bus, @http/client, @http/headers, lodash
    let names: Vec<&str> = output.packages.iter().map(|p| p.name.as_str()).collect();
    assert_eq!(
        output.packages.len(),
        10,
        "Expected 10 packages, got {}: {names:?}",
        output.packages.len()
    );
}

#[test]
fn test_large_app_no_duplicates() {
    let output = trace(TraceOptions {
        app_root: fixtures_dir().join("large-app"),
        entries: vec![PathBuf::from("src/index.ts"), PathBuf::from("src/admin.ts")],
    })
    .expect("Trace should succeed");

    let mut seen = std::collections::HashSet::new();
    for pkg in &output.packages {
        let key = format!("{}@{}", pkg.name, pkg.version);
        assert!(seen.insert(key.clone()), "Duplicate package: {key}");
    }
}

// ── pnpm layout ─────────────────────────────────────────────────────

#[test]
fn test_pnpm_symlink_layout() {
    let output = trace(TraceOptions {
        app_root: fixtures_dir().join("pnpm-app"),
        entries: vec![PathBuf::from("src/index.ts")],
    })
    .expect("Trace should succeed with pnpm symlinked layout");

    let react = output
        .packages
        .iter()
        .find(|p| p.name == "react")
        .expect("react should be discovered through symlink");
    assert_eq!(react.version, "18.0.0");

    let scheduler = output
        .packages
        .iter()
        .find(|p| p.name == "scheduler")
        .expect("scheduler should be discovered through nested symlink");
    assert_eq!(scheduler.version, "0.23.0");

    // react should appear exactly once (no duplicates from symlink vs real path).
    let react_count = output.packages.iter().filter(|p| p.name == "react").count();
    assert_eq!(react_count, 1, "react should not be duplicated by symlinks");
}

// ── Nested versions ─────────────────────────────────────────────────

#[test]
fn test_nested_versions() {
    let output = trace(TraceOptions {
        app_root: fixtures_dir().join("nested-versions-app"),
        entries: vec![PathBuf::from("src/index.ts")],
    })
    .expect("Trace should succeed");

    // Both lib-a and lib-b should be discovered.
    assert!(output.packages.iter().any(|p| p.name == "lib-a"));
    assert!(output.packages.iter().any(|p| p.name == "lib-b"));

    // Two different versions of util should exist.
    let utils: Vec<_> = output
        .packages
        .iter()
        .filter(|p| p.name == "util")
        .collect();
    assert_eq!(
        utils.len(),
        2,
        "Expected 2 versions of util, got {}: {:?}",
        utils.len(),
        utils.iter().map(|u| &u.version).collect::<Vec<_>>()
    );

    let versions: std::collections::HashSet<&str> =
        utils.iter().map(|u| u.version.as_str()).collect();
    assert!(versions.contains("1.0.0"), "util@1.0.0 should exist");
    assert!(versions.contains("2.0.0"), "util@2.0.0 should exist");
}
