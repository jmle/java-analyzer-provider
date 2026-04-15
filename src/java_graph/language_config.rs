// tree-sitter-java configuration

use anyhow::{Context, Result};
use std::path::Path;
use tree_sitter::{Language, Parser, Tree};

/// Get the tree-sitter Java language
pub fn language() -> Language {
    tree_sitter_java::LANGUAGE.into()
}

/// Create a new parser configured for Java
pub fn create_parser() -> Result<Parser> {
    let mut parser = Parser::new();
    parser
        .set_language(&language())
        .context("Failed to set Java language for parser")?;
    Ok(parser)
}

/// Parse a Java source file
pub fn parse_file(path: &Path) -> Result<(String, Tree)> {
    let source = std::fs::read_to_string(path)
        .with_context(|| format!("Failed to read file: {}", path.display()))?;

    let tree = parse_source(&source)?;

    Ok((source, tree))
}

/// Parse Java source code from a string
pub fn parse_source(source: &str) -> Result<Tree> {
    let mut parser = create_parser()?;

    parser
        .parse(source, None)
        .context("Failed to parse Java source")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parser_creation() {
        let parser = create_parser();
        assert!(parser.is_ok());
    }

    #[test]
    fn test_parse_simple_class() {
        let source = r#"
            public class HelloWorld {
                public static void main(String[] args) {
                    System.out.println("Hello, World!");
                }
            }
        "#;

        let tree = parse_source(source);
        assert!(tree.is_ok());

        let tree = tree.unwrap();
        let root = tree.root_node();

        // Root should be a program node
        assert_eq!(root.kind(), "program");
        assert!(root.child_count() > 0);
    }

    #[test]
    fn test_parse_with_package() {
        let source = r#"
            package com.example.app;

            public class MyClass {
                private int value;
            }
        "#;

        let tree = parse_source(source);
        assert!(tree.is_ok());
    }

    #[test]
    fn test_parse_with_imports() {
        let source = r#"
            package com.example;

            import java.util.List;
            import java.util.ArrayList;

            public class Example {
            }
        "#;

        let tree = parse_source(source);
        assert!(tree.is_ok());
    }
}
