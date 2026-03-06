use std::path::PathBuf;
use std::process;

use compiler::{CompileOptions, compile};

fn main() {
    let args: Vec<String> = std::env::args().collect();

    if args.len() < 3 {
        eprintln!("Usage: cp <package_root> <entry1> [entry2] ... [--source-maps]");
        eprintln!("Example: cp ./node_modules/react src/index.ts");
        process::exit(1);
    }

    let source_maps = args.iter().any(|a| a == "--source-maps");
    let positional: Vec<&String> = args[1..].iter().filter(|a| !a.starts_with("--")).collect();

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

            // Write manifest.
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
