use std::path::PathBuf;
use std::process;

use compiler::{CompileOptions, compile};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    if args[1] == "build" {
        run_build(&args[2..]);
    } else {
        run_compile(&args[1..]);
    }
}

fn print_usage() {
    eprintln!("Usage:");
    eprintln!("  cp <package_root> <entry1> [entry2] ... [--source-maps]");
    eprintln!("  cp build <app_root> <entry1> [entry2] ... [--source-maps]");
}

fn run_compile(args: &[String]) {
    let source_maps = args.iter().any(|a| a == "--source-maps");
    let positional: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();

    if positional.len() < 2 {
        eprintln!("Usage: cp <package_root> <entry1> [entry2] ... [--source-maps]");
        process::exit(1);
    }

    let package_root = PathBuf::from(positional[0]);
    let entries: Vec<PathBuf> = positional[1..].iter().map(PathBuf::from).collect();
    let output_dir = package_root.join("dist");

    match compile(CompileOptions {
        package_root: package_root.clone(),
        entries,
        source_maps,
    }) {
        Ok(output) => {
            std::fs::create_dir_all(&output_dir).unwrap();

            for file in &output.files {
                let path = output_dir.join(&file.name);
                std::fs::write(&path, &file.content).unwrap();
                println!("  {}", path.display());

                if let Some(ref map) = file.source_map {
                    let map_path = output_dir.join(format!("{}.map", file.name));
                    std::fs::write(&map_path, map).unwrap();
                    println!("  {}", map_path.display());
                }
            }

            let manifest_path = output_dir.join("manifest.json");
            let manifest_json = serde_json::to_string_pretty(&output.manifest).unwrap();
            std::fs::write(&manifest_path, &manifest_json).unwrap();
            println!("  {}", manifest_path.display());
        }
        Err(e) => {
            eprintln!("Error: {e}");
            process::exit(1);
        }
    }
}

fn run_build(args: &[String]) {
    let source_maps = args.iter().any(|a| a == "--source-maps");
    let positional: Vec<&String> = args.iter().filter(|a| !a.starts_with("--")).collect();

    if positional.len() < 2 {
        eprintln!("Usage: cp build <app_root> <entry1> [entry2] ... [--source-maps]");
        process::exit(1);
    }

    let app_root = PathBuf::from(positional[0]);
    let entries: Vec<PathBuf> = positional[1..].iter().map(PathBuf::from).collect();
    let dist_dir = app_root.join("dist");

    match builder::build(builder::BuildOptions {
        app_root: app_root.clone(),
        entries,
        source_maps,
    }) {
        Ok(output) => {
            std::fs::create_dir_all(&dist_dir).unwrap();

            // Write app files under _app/.
            let app_dir = dist_dir.join("_app");
            std::fs::create_dir_all(&app_dir).unwrap();
            for file in &output.app.files {
                let path = app_dir.join(&file.name);
                std::fs::write(&path, &file.content).unwrap();
                println!("  {}", path.display());

                if let Some(ref map) = file.source_map {
                    let map_path = app_dir.join(format!("{}.map", file.name));
                    std::fs::write(&map_path, map).unwrap();
                    println!("  {}", map_path.display());
                }
            }

            // Write package files under {name}@{version}/.
            for pkg in &output.packages {
                let pkg_dir = dist_dir.join(format!("{}@{}", pkg.name, pkg.version));
                std::fs::create_dir_all(&pkg_dir).unwrap();
                for file in &pkg.files {
                    let path = pkg_dir.join(&file.name);
                    std::fs::write(&path, &file.content).unwrap();
                    println!("  {}", path.display());

                    if let Some(ref map) = file.source_map {
                        let map_path = pkg_dir.join(format!("{}.map", file.name));
                        std::fs::write(&map_path, map).unwrap();
                        println!("  {}", map_path.display());
                    }
                }
            }

            // Write import-map.json.
            let import_map_path = dist_dir.join("import-map.json");
            let import_map_json = serde_json::to_string_pretty(&output.import_map).unwrap();
            std::fs::write(&import_map_path, &import_map_json).unwrap();
            println!("  {}", import_map_path.display());
        }
        Err(e) => {
            eprintln!("Error: {e}");
            process::exit(1);
        }
    }
}
