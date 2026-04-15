use java_analyzer_provider::java_graph::{ast_explorer, language_config};

#[test]
#[ignore]
fn explore_advanced_features() {
    let source = std::fs::read_to_string("tests/fixtures/AdvancedFeatures.java").unwrap();
    let tree = language_config::parse_source(&source).unwrap();

    println!("\n=== Full AST (depth 4) ===");
    ast_explorer::print_ast(&tree, &source, Some(4));
}

#[test]
#[ignore]
fn find_specific_nodes() {
    let source = std::fs::read_to_string("tests/fixtures/AdvancedFeatures.java").unwrap();
    let tree = language_config::parse_source(&source).unwrap();

    println!("\n=== Superclass nodes ===");
    let superclass_nodes = ast_explorer::find_nodes_by_kind(&tree, "superclass");
    for node in &superclass_nodes {
        println!("{}", ast_explorer::node_text(*node, &source));
    }

    println!("\n=== Super interfaces nodes ===");
    let interface_nodes = ast_explorer::find_nodes_by_kind(&tree, "super_interfaces");
    for node in &interface_nodes {
        println!("{}", ast_explorer::node_text(*node, &source));
    }

    println!("\n=== Annotation nodes ===");
    let annotation_nodes = ast_explorer::find_nodes_by_kind(&tree, "marker_annotation");
    for node in &annotation_nodes {
        println!("{}", ast_explorer::node_text(*node, &source));
    }

    let annotation_nodes2 = ast_explorer::find_nodes_by_kind(&tree, "annotation");
    for node in &annotation_nodes2 {
        println!("{}", ast_explorer::node_text(*node, &source));
    }

    println!("\n=== Method invocation nodes ===");
    let method_call_nodes = ast_explorer::find_nodes_by_kind(&tree, "method_invocation");
    for (i, node) in method_call_nodes.iter().take(5).enumerate() {
        println!("{}. {}", i+1, ast_explorer::node_text(*node, &source));
    }

    println!("\n=== Object creation nodes ===");
    let new_nodes = ast_explorer::find_nodes_by_kind(&tree, "object_creation_expression");
    for node in &new_nodes {
        println!("{}", ast_explorer::node_text(*node, &source));
    }
}
