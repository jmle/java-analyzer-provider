// AST exploration utilities for debugging and development

use tree_sitter::{Node, Tree};

/// Print the AST structure for debugging
pub fn print_ast(tree: &Tree, source: &str, max_depth: Option<usize>) {
    let root = tree.root_node();
    print_node(root, source, 0, max_depth.unwrap_or(usize::MAX));
}

fn print_node(node: Node, source: &str, depth: usize, max_depth: usize) {
    if depth > max_depth {
        return;
    }

    let indent = "  ".repeat(depth);
    let kind = node.kind();
    let start = node.start_position();
    let end = node.end_position();

    // Get the text for leaf nodes or small nodes
    let text = if node.child_count() == 0 || (end.row == start.row && end.column - start.column < 50) {
        let text = node.utf8_text(source.as_bytes()).unwrap_or("");
        format!(" [{}]", text.replace('\n', "\\n"))
    } else {
        String::new()
    };

    println!(
        "{}{} ({}:{} - {}:{}){}",
        indent, kind, start.row, start.column, end.row, end.column, text
    );

    // Print children
    for child in node.children(&mut node.walk()) {
        print_node(child, source, depth + 1, max_depth);
    }
}

/// Find all nodes of a specific kind in the tree
pub fn find_nodes_by_kind<'a>(tree: &'a Tree, kind: &str) -> Vec<Node<'a>> {
    let mut nodes = Vec::new();
    let root = tree.root_node();
    find_nodes_recursive(root, kind, &mut nodes);
    nodes
}

fn find_nodes_recursive<'a>(node: Node<'a>, kind: &str, nodes: &mut Vec<Node<'a>>) {
    if node.kind() == kind {
        nodes.push(node);
    }

    for child in node.children(&mut node.walk()) {
        find_nodes_recursive(child, kind, nodes);
    }
}

/// Get the text content of a node
pub fn node_text<'a>(node: Node, source: &'a str) -> &'a str {
    node.utf8_text(source.as_bytes()).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::java_graph::language_config;

    #[test]
    fn test_find_nodes() {
        let source = r#"
            package com.test;

            public class Test {
                private int field;
            }
        "#;

        let tree = language_config::parse_source(source).unwrap();

        // Find package declaration
        let packages = find_nodes_by_kind(&tree, "package_declaration");
        assert_eq!(packages.len(), 1);

        // Find class declaration
        let classes = find_nodes_by_kind(&tree, "class_declaration");
        assert_eq!(classes.len(), 1);

        // Find field declaration
        let fields = find_nodes_by_kind(&tree, "field_declaration");
        assert_eq!(fields.len(), 1);
    }

    #[test]
    fn test_node_text() {
        let source = r#"
            package com.test;

            public class Test {
            }
        "#;

        let tree = language_config::parse_source(source).unwrap();
        let packages = find_nodes_by_kind(&tree, "package_declaration");

        assert_eq!(packages.len(), 1);
        let package_text = node_text(packages[0], source);
        assert!(package_text.contains("package com.test"));
    }
}
