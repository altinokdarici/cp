use oxc_ast::ast::{
    CallExpression, ExportAllDeclaration, ExportNamedDeclaration, Expression, ImportDeclaration,
    ImportExpression,
};
use oxc_ast_visit::Visit;

/// Lightweight import collector that only extracts specifier strings.
/// Handles ES imports/exports, CommonJS require(), and dynamic import().
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

    fn visit_call_expression(&mut self, expr: &CallExpression<'a>) {
        // Collect require("specifier") calls.
        if let Expression::Identifier(callee) = &expr.callee
            && callee.name.as_str() == "require"
            && let Some(first_arg) = expr.arguments.first()
            && let Some(Expression::StringLiteral(lit)) = first_arg.as_expression()
        {
            self.specifiers.push(lit.value.to_string());
        }
        // Continue walking to find nested require() calls.
        for arg in &expr.arguments {
            self.visit_argument(arg);
        }
    }

    fn visit_import_expression(&mut self, expr: &ImportExpression<'a>) {
        // Collect dynamic import("specifier") calls.
        if let Expression::StringLiteral(lit) = &expr.source {
            self.specifiers.push(lit.value.to_string());
        }
    }
}
