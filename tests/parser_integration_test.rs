use java_analyzer_provider::java_graph::language_config;
use std::path::Path;

#[test]
fn test_parse_simple_fixture() {
    let fixture_path = Path::new("tests/fixtures/Simple.java");
    assert!(
        fixture_path.exists(),
        "Fixture file not found: {}",
        fixture_path.display()
    );

    let result = language_config::parse_file(fixture_path);
    assert!(result.is_ok(), "Failed to parse Simple.java");

    let (source, tree) = result.unwrap();

    // Verify we got the source
    assert!(source.contains("package com.example.simple"));
    assert!(source.contains("public class Simple"));

    // Verify the tree structure
    let root = tree.root_node();
    assert_eq!(root.kind(), "program");

    // Find package declaration
    let mut found_package = false;
    let mut found_class = false;

    for child in root.children(&mut root.walk()) {
        match child.kind() {
            "package_declaration" => found_package = true,
            "class_declaration" => found_class = true,
            _ => {}
        }
    }

    assert!(found_package, "Package declaration not found in AST");
    assert!(found_class, "Class declaration not found in AST");
}

#[test]
fn test_parse_source_directly() {
    let source = r#"
        package com.test;

        import java.util.Map;

        public class TestClass extends BaseClass implements Runnable {
            private int field;

            @Override
            public void run() {
                System.out.println("Running");
            }

            public int getField() {
                return field;
            }
        }
    "#;

    let result = language_config::parse_source(source);
    assert!(result.is_ok());

    let tree = result.unwrap();
    let root = tree.root_node();

    // Check for various node types
    let mut found_import = false;
    let mut found_annotation = false;

    fn walk_tree(node: tree_sitter::Node, found_import: &mut bool, found_annotation: &mut bool) {
        match node.kind() {
            "import_declaration" => *found_import = true,
            "marker_annotation" => *found_annotation = true,
            _ => {}
        }

        for child in node.children(&mut node.walk()) {
            walk_tree(child, found_import, found_annotation);
        }
    }

    walk_tree(root, &mut found_import, &mut found_annotation);

    assert!(found_import, "Import declaration not found");
    assert!(found_annotation, "Annotation not found");
}
