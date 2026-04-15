use java_analyzer_provider::java_graph::loader;
use std::path::PathBuf;

#[test]
fn test_build_graph_with_advanced_features() {
    let fixture_path = PathBuf::from("tests/fixtures/AdvancedFeatures.java");
    if !fixture_path.exists() {
        eprintln!("Skipping test - fixture not found");
        return;
    }

    let result = loader::build_graph_for_files(&[&fixture_path]);
    assert!(result.is_ok(), "Failed to build graph: {:?}", result.err());

    let graph = result.unwrap();

    // Verify the graph was created
    assert!(graph.iter_files().count() > 0);

    // The graph should contain nodes for the advanced features
    // (inheritance, implements, method calls, constructor calls, annotations)
    let node_count = graph.iter_nodes().count();
    println!("Graph has {} nodes", node_count);
    assert!(node_count > 0);
}

#[test]
fn test_build_graph_with_inheritance() {
    let fixture_path = PathBuf::from("tests/fixtures/InheritanceExample.java");
    if !fixture_path.exists() {
        eprintln!("Skipping test - fixture not found");
        return;
    }

    let result = loader::build_graph_for_files(&[&fixture_path]);
    assert!(result.is_ok(), "Failed to build graph: {:?}", result.err());

    let graph = result.unwrap();
    assert!(graph.iter_files().count() > 0);

    let node_count = graph.iter_nodes().count();
    println!("Inheritance graph has {} nodes", node_count);
    assert!(node_count > 0);
}

#[test]
fn test_build_graph_with_method_calls() {
    let fixture_path = PathBuf::from("tests/fixtures/MethodCallExample.java");
    if !fixture_path.exists() {
        eprintln!("Skipping test - fixture not found");
        return;
    }

    let result = loader::build_graph_for_files(&[&fixture_path]);
    assert!(result.is_ok(), "Failed to build graph: {:?}", result.err());

    let graph = result.unwrap();
    assert!(graph.iter_files().count() > 0);

    let node_count = graph.iter_nodes().count();
    println!("Method call graph has {} nodes", node_count);
    assert!(node_count > 0);
}

#[test]
fn test_build_graph_with_annotations() {
    let fixture_path = PathBuf::from("tests/fixtures/AnnotationExample.java");
    if !fixture_path.exists() {
        eprintln!("Skipping test - fixture not found");
        return;
    }

    let result = loader::build_graph_for_files(&[&fixture_path]);
    assert!(result.is_ok(), "Failed to build graph: {:?}", result.err());

    let graph = result.unwrap();
    assert!(graph.iter_files().count() > 0);

    let node_count = graph.iter_nodes().count();
    println!("Annotation graph has {} nodes", node_count);
    assert!(node_count > 0);
}
