use java_analyzer_provider::java_graph::loader;
use std::path::PathBuf;

#[test]
fn verify_inheritance_in_graph() {
    let source = r#"
        package com.test;

        public class Child extends Parent implements Interface1, Interface2 {
            public void method() {
            }
        }
    "#;

    use tempfile::NamedTempFile;
    use std::io::Write;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(source.as_bytes()).unwrap();
    let temp_path = temp_file.path().to_path_buf();

    let result = loader::build_graph_for_files(&[&temp_path]);
    assert!(result.is_ok(), "Failed to build graph: {:?}", result.err());

    let graph = result.unwrap();

    // Count nodes - should have nodes for:
    // - program, package, class, method
    // - parent ref (inheritance)
    // - interface refs (implements)
    let node_count = graph.iter_nodes().count();
    println!("Inheritance test: {} nodes", node_count);

    // Should have at least: program, package, class, parent ref, 2 interface refs, method
    assert!(node_count >= 7, "Expected at least 7 nodes, got {}", node_count);
}

#[test]
fn verify_method_calls_in_graph() {
    let source = r#"
        package com.test;

        public class TestClass {
            public void example() {
                service.doSomething();
                System.out.println("test");
                obj.method1().method2();
            }
        }
    "#;

    use tempfile::NamedTempFile;
    use std::io::Write;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(source.as_bytes()).unwrap();
    let temp_path = temp_file.path().to_path_buf();

    let result = loader::build_graph_for_files(&[&temp_path]);
    assert!(result.is_ok(), "Failed to build graph: {:?}", result.err());

    let graph = result.unwrap();

    // Should have nodes for method invocations
    let node_count = graph.iter_nodes().count();
    println!("Method calls test: {} nodes", node_count);

    // Should have: program, package, class, method, and method call nodes
    assert!(node_count >= 8, "Expected at least 8 nodes for method calls, got {}", node_count);
}

#[test]
fn verify_constructor_calls_in_graph() {
    let source = r#"
        package com.test;

        public class TestClass {
            public void example() {
                User user = new User("test");
                List<String> list = new ArrayList<>();
            }
        }
    "#;

    use tempfile::NamedTempFile;
    use std::io::Write;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(source.as_bytes()).unwrap();
    let temp_path = temp_file.path().to_path_buf();

    let result = loader::build_graph_for_files(&[&temp_path]);
    assert!(result.is_ok(), "Failed to build graph: {:?}", result.err());

    let graph = result.unwrap();

    // Should have nodes for constructor calls
    let node_count = graph.iter_nodes().count();
    println!("Constructor calls test: {} nodes", node_count);

    // Should have: program, package, class, method, and constructor call nodes
    assert!(node_count >= 7, "Expected at least 7 nodes for constructor calls, got {}", node_count);
}

#[test]
fn verify_annotations_in_graph() {
    let source = r#"
        package com.test;

        @Service
        @Component(value = "myComponent")
        public class TestClass {
            @Autowired
            private UserService service;

            @Override
            public void method() {
            }
        }
    "#;

    use tempfile::NamedTempFile;
    use std::io::Write;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(source.as_bytes()).unwrap();
    let temp_path = temp_file.path().to_path_buf();

    let result = loader::build_graph_for_files(&[&temp_path]);
    assert!(result.is_ok(), "Failed to build graph: {:?}", result.err());

    let graph = result.unwrap();

    // Should have nodes for annotations
    let node_count = graph.iter_nodes().count();
    println!("Annotations test: {} nodes", node_count);

    // Should have: program, package, class, field, method, and annotation nodes
    assert!(node_count >= 10, "Expected at least 10 nodes for annotations, got {}", node_count);
}

#[test]
fn verify_all_advanced_features_together() {
    let fixture_path = PathBuf::from("tests/fixtures/AdvancedFeatures.java");
    if !fixture_path.exists() {
        eprintln!("Skipping test - fixture not found");
        return;
    }

    let result = loader::build_graph_for_files(&[&fixture_path]);
    assert!(result.is_ok(), "Failed to build graph: {:?}", result.err());

    let graph = result.unwrap();

    let node_count = graph.iter_nodes().count();
    println!("All features test: {} nodes", node_count);

    // AdvancedFeatures.java has:
    // - 1 package, 1 class with extends + 2 implements
    // - Multiple fields and methods
    // - Multiple annotations
    // - Multiple method calls
    // - Multiple constructor calls
    // Should have many nodes
    assert!(node_count >= 30, "Expected at least 30 nodes for complete file, got {}", node_count);
}
