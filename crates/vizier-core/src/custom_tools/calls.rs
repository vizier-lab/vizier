//! The functions a script calls (`specs/061-agent-custom-tools` research R4): parsed with the
//! same Python parser the sandbox's engine is built on, so strings, comments and attribute
//! calls never count. A call is a `Call` whose function is a bare name the script does not
//! bind itself (assignment, `def`, `class`, a parameter, an import, a loop or `with` target) and
//! that is not a Python builtin.

use std::collections::BTreeSet;

use ruff_python_ast::{
    self as ast, Expr, ExprContext, Stmt,
    visitor::{self, Visitor},
};

/// Where a script stops parsing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntaxProblem {
    /// 1-based.
    pub line: usize,
    /// 1-based, in characters.
    pub column: usize,
    pub message: String,
}

/// Builtins and sandbox helpers a script may call that are not tools.
const NOT_TOOLS: &[&str] = &[
    // Python builtins the sandbox supports.
    "abs", "all", "any", "ascii", "bin", "bool", "bytearray", "bytes", "callable", "chr",
    "complex", "dict", "dir", "divmod", "enumerate", "filter", "float", "format", "frozenset",
    "getattr", "globals", "hasattr", "hash", "hex", "id", "int", "isinstance", "issubclass",
    "iter", "len", "list", "locals", "map", "max", "min", "next", "object", "oct", "ord", "pow",
    "print", "range", "repr", "reversed", "round", "set", "setattr", "slice", "sorted", "str",
    "sum", "super", "tuple", "type", "vars", "zip",
    // Exceptions a script raises.
    "ArithmeticError", "AssertionError", "AttributeError", "BaseException", "Exception",
    "IndexError", "KeyError", "LookupError", "MemoryError", "NameError", "NotImplementedError",
    "OSError", "OverflowError", "RecursionError", "RuntimeError", "StopIteration", "TimeoutError",
    "TypeError", "ValueError", "ZeroDivisionError",
    // The sandbox's own discovery helpers.
    "list_tools", "describe_tool",
];

/// The tool names `script` calls, sorted, or where it fails to parse.
pub fn parse_calls(script: &str) -> Result<Vec<String>, SyntaxProblem> {
    let parsed = ruff_python_parser::parse_module(script).map_err(|error| {
        let offset = error.location.start().to_usize();
        let (line, column) = line_column(script, offset);
        SyntaxProblem { line, column, message: error.error.to_string() }
    })?;
    let mut finder = Finder::default();
    finder.visit_body(&parsed.syntax().body);
    let calls = finder
        .called
        .into_iter()
        .filter(|name| !finder.bound.contains(name) && !NOT_TOOLS.contains(&name.as_str()))
        .collect();
    Ok(calls)
}

/// 1-based line and column of a byte offset.
fn line_column(source: &str, offset: usize) -> (usize, usize) {
    let offset = offset.min(source.len());
    let before = &source[..offset];
    let line = before.matches('\n').count() + 1;
    let column = before.rsplit('\n').next().unwrap_or("").chars().count() + 1;
    (line, column)
}

#[derive(Default)]
struct Finder {
    called: BTreeSet<String>,
    bound: BTreeSet<String>,
}

impl<'a> Visitor<'a> for Finder {
    fn visit_stmt(&mut self, stmt: &'a Stmt) {
        match stmt {
            Stmt::FunctionDef(def) => {
                self.bound.insert(def.name.as_str().to_string());
            }
            Stmt::ClassDef(def) => {
                self.bound.insert(def.name.as_str().to_string());
            }
            Stmt::Import(import) => {
                for alias in &import.names {
                    self.bind_alias(alias);
                }
            }
            Stmt::ImportFrom(import) => {
                for alias in &import.names {
                    self.bind_alias(alias);
                }
            }
            _ => {}
        }
        visitor::walk_stmt(self, stmt);
    }

    fn visit_expr(&mut self, expr: &'a Expr) {
        match expr {
            Expr::Call(call) => {
                if let Expr::Name(name) = call.func.as_ref() {
                    self.called.insert(name.id.as_str().to_string());
                }
            }
            Expr::Name(name) if matches!(name.ctx, ExprContext::Store | ExprContext::Del) => {
                self.bound.insert(name.id.as_str().to_string());
            }
            _ => {}
        }
        visitor::walk_expr(self, expr);
    }

    fn visit_parameter(&mut self, parameter: &'a ast::Parameter) {
        self.bound.insert(parameter.name.as_str().to_string());
        visitor::walk_parameter(self, parameter);
    }

    fn visit_except_handler(&mut self, handler: &'a ast::ExceptHandler) {
        let ast::ExceptHandler::ExceptHandler(handler_inner) = handler;
        if let Some(name) = &handler_inner.name {
            self.bound.insert(name.as_str().to_string());
        }
        visitor::walk_except_handler(self, handler);
    }
}

impl Finder {
    fn bind_alias(&mut self, alias: &ast::Alias) {
        let bound = alias.asname.as_ref().unwrap_or(&alias.name);
        // `import a.b` binds `a`.
        let first = bound.as_str().split('.').next().unwrap_or_default();
        self.bound.insert(first.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn calls(script: &str) -> Vec<String> {
        parse_calls(script).unwrap()
    }

    #[test]
    fn bare_name_calls_are_found_nested_and_sorted() {
        assert_eq!(
            calls("x = memory_search(query=memory_read(bundle='a', path='b')['t'])\nx"),
            vec!["memory_read", "memory_search"]
        );
    }

    #[test]
    fn strings_comments_attributes_builtins_and_bound_names_do_not_count() {
        let script = r#"
import json
# memory_write(bundle="x")
s = "memory_delete()"
def helper(n):
    return len(str(n))
f = custom_other
data = json.loads("{}")
for item in sorted([3, 1]):
    print(helper(item))
try:
    current_time()
except ValueError as err:
    raise RuntimeError(str(err))
f()
data
"#;
        assert_eq!(calls(script), vec!["current_time"]);
    }

    #[test]
    fn a_syntax_error_has_its_position() {
        let problem = parse_calls("x = 1\ny = memory_serch(query=1").unwrap_err();
        assert_eq!(problem.line, 2);
        assert!(problem.column >= 1);
        assert!(!problem.message.is_empty());
    }
}
