use std::collections::BTreeMap;

use serde::Serialize;

use crate::builder::CompiledPackage;

/// An import map mapping bare specifiers to virtual paths.
/// Serialized as `{ "imports": { "react": "/@cp/react@18.0.0/index.js" } }`.
#[derive(Debug, Serialize)]
pub struct ImportMap {
    /// Bare specifier → virtual path (e.g. "react" → "/@cp/react@18.0.0/index.js").
    pub imports: BTreeMap<String, String>,
}

/// Generate an import map from compiled packages.
///
/// Each bare specifier that was resolved to a package gets mapped to
/// `/@cp/{name}@{version}/{output_file}`. The output file name comes from
/// the compiler manifest entries, positionally correlated with the input entries.
pub fn generate_import_map(
    packages: &[CompiledPackage],
    specifier_to_entry: &BTreeMap<String, (String, usize)>,
) -> ImportMap {
    let mut imports = BTreeMap::new();

    for (specifier, (pkg_key, entry_idx)) in specifier_to_entry {
        // Look up the compiled package by its "name@version" key.
        if let Some(pkg) = packages
            .iter()
            .find(|p| format!("{}@{}", p.name, p.version) == *pkg_key)
            // Correlate by position: entry_idx in the specifier map matches the manifest entries order.
            && let Some(output_name) = pkg.manifest.entries.get(*entry_idx)
        {
            // Combine the virtual prefix with the output file name.
            let virtual_path = format!("{}{}", pkg.virtual_prefix, output_name);
            imports.insert(specifier.clone(), virtual_path);
        }
    }

    ImportMap { imports }
}

/// Build the virtual prefix for a package: `/@cp/{name}@{version}/`.
/// Used as the base URL path for all files in this package.
pub fn virtual_prefix(name: &str, version: &str) -> String {
    format!("/@cp/{name}@{version}/")
}
