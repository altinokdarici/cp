use oxc_ast::ast::{ExportAllDeclaration, ExportNamedDeclaration, ImportDeclaration};
use oxc_ast_visit::Visit;

/// Lightweight import collector that only extracts specifier strings.
/// No binding info — just the bare minimum for dependency tracing.
#[derive(Default)]
pub struct ImportCollector {
    pub specifiers: Vec<String>,
}

impl<'a> Visit<'a> for ImportCollector {
    fn visit_import_declaration(&mut self, decl: &ImportDeclaration<'a>) {
        self.specifiers.push(decl.source.value.to_string());
    }

    fn visit_export_all_declaration(&mut self, decl: &ExportAllDeclaration<'a>) {
        self.specifiers.push(decl.source.value.to_string());
    }

    fn visit_export_named_declaration(&mut self, decl: &ExportNamedDeclaration<'a>) {
        if let Some(source) = &decl.source {
            self.specifiers.push(source.value.to_string());
        }
    }
}
