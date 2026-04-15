use java_analyzer_provider::java_graph::{ast_explorer, language_config};

#[test]
#[ignore] // Run with --ignored to see AST output
fn explore_simple_java() {
    let source = r#"
        package com.example.simple;

        import java.util.List;

        public class Simple {
            private int value;

            public Simple(int value) {
                this.value = value;
            }

            public int getValue() {
                return value;
            }
        }
    "#;

    let tree = language_config::parse_source(source).unwrap();
    ast_explorer::print_ast(&tree, source, Some(5));
}

#[test]
fn find_node_types() {
    let source = r#"
        package com.example.simple;

        import java.util.List;

        public class Simple {
            private int value;

            public Simple(int value) {
                this.value = value;
            }

            public int getValue() {
                return value;
            }
        }
    "#;

    let tree = language_config::parse_source(source).unwrap();
    let root = tree.root_node();

    fn collect_node_types(node: tree_sitter::Node, types: &mut std::collections::HashSet<String>) {
        types.insert(node.kind().to_string());
        for child in node.children(&mut node.walk()) {
            collect_node_types(child, types);
        }
    }

    let mut node_types = std::collections::HashSet::new();
    collect_node_types(root, &mut node_types);

    let mut sorted: Vec<_> = node_types.iter().collect();
    sorted.sort();

    println!("\nAll node types found in the Java AST:");
    for node_type in sorted {
        println!("  - {}", node_type);
    }
}
