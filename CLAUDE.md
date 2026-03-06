# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

CP is a per-package ESM compiler written in Rust. It takes npm package entry points, traces imports, transforms TS/JSX to JS via the oxc toolchain, splits shared modules into chunks, and emits bundled output files with a manifest.

## Build & Test Commands

```bash
cargo build                          # Dev build
cargo build --release                # Release build
cargo test                           # Run all tests
cargo test <test_name>               # Run a single test
cargo bench --bench compile_bench    # Run performance benchmarks
```

The CLI binary is named `cp`: `cp <package_root> <entry1> [entry2] ...`

## Architecture

Two crates in a Rust workspace (edition 2024):

- **`crates/compiler`** — core library, the public API is `compile(CompileOptions) -> Result<CompileOutput, String>`
- **`crates/cli`** — thin CLI wrapper that calls `compile()` and writes output files + `manifest.json` to `<package_root>/dist/`

### Compiler Pipeline (`compiler.rs` → `graph.rs` → `linker.rs`)

1. **Graph build** (`graph.rs`): Two-phase module graph construction
   - *Phase 1 — Sequential discovery*: DFS from entries. For each module: read file → `loader::load()` → oxc parse (lightweight, imports only) → resolve with `oxc_resolver` → recurse. Detects circular dependencies.
   - *Phase 2 — Parallel transform*: If 32+ modules need transformation, uses `rayon::par_iter`; otherwise sequential. Each module: oxc parse → `SemanticBuilder` → `Transformer` (TS/JSX strip) → `Codegen`.

2. **Linking** (`linker.rs`): Computes chunk plan via DFS reachability analysis. Modules reachable from 2+ entries go into `chunk-shared.js`. Exclusive modules stay with their entry. Strips import statements, deduplicates external imports, concatenates module bodies.

3. **Loading** (`loader.rs`): Maps file extensions to handling. JS/TS files pass through (TS/JSX flagged for transform). JSON/CSS/GraphQL/SVG/TXT/HTML get wrapped in `export default`.

### Key Types

- `CompileOptions` — `package_root: PathBuf` + `entries: Vec<PathBuf>`
- `CompileOutput` — `files: Vec<OutputFile>` + `manifest: Manifest`
- `Manifest` — `entries`, `chunks`, `externals` (all `Vec<String>`, serialized to JSON)
- `ModuleGraph` — `modules: HashMap<PathBuf, Module>` + `entries: Vec<PathBuf>`

### Import Classification

- **Relative** (`./` or `../`) → resolved via `oxc_resolver`, traced as internal dependency
- **Bare specifier** (anything else) → treated as external, listed in manifest

## Git Commits

Use conventional commits (e.g., `feat:`, `fix:`, `refactor:`, `perf:`, `test:`, `chore:`, `docs:`). Always single-line commit messages. No AI attribution — do not add "Co-Authored-By" or any AI tool references.

## Performance

A custom agent exists at `.claude/agents/perf-expert.md` for performance analysis. The benchmark (`benches/compile_bench.rs`) generates synthetic packages of varying sizes (20/100/500 modules) and measures end-to-end compile time.

Key optimizations in place: single-pass parse+transform+codegen, adaptive parallelism threshold, HashSet dedup, pre-allocated string buffers, owned-String move semantics in the loader.
