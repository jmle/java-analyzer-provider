use java_analyzer_provider::java_graph::type_resolver::TypeResolver;
use std::path::PathBuf;

#[test]
fn test_analyze_all_fixtures() {
    let fixtures = [
        "tests/fixtures/Simple.java",
        "tests/fixtures/InheritanceExample.java",
        "tests/fixtures/MethodCallExample.java",
        "tests/fixtures/AnnotationExample.java",
    ];

    let mut resolver = TypeResolver::new();

    for fixture in &fixtures {
        let path = PathBuf::from(fixture);
        if path.exists() {
            let result = resolver.analyze_file(&path);
            assert!(result.is_ok(), "Failed to analyze {}: {:?}", fixture, result.err());
        }
    }

    // Verify we analyzed the files
    assert!(resolver.file_infos.len() > 0);
}

#[test]
fn test_global_index_build() {
    let fixtures = [
        "tests/fixtures/Simple.java",
        "tests/fixtures/InheritanceExample.java",
    ];

    let mut resolver = TypeResolver::new();

    for fixture in &fixtures {
        let path = PathBuf::from(fixture);
        if path.exists() {
            resolver.analyze_file(&path).unwrap();
        }
    }

    resolver.build_global_index();

    // Check that classes are in the index
    if resolver.file_infos.len() > 0 {
        assert!(resolver.global_type_index.contains_key("Simple") ||
                resolver.global_type_index.contains_key("InheritanceExample"));
    }
}

#[test]
fn test_wildcard_resolution_with_global_index() {
    // Create a test file with wildcard import
    let source = r#"
        package com.test;

        import java.util.*;

        public class WildcardTest {
            private List items;
        }
    "#;

    use java_analyzer_provider::java_graph::language_config;
    use tempfile::NamedTempFile;
    use std::io::Write;

    // Write to temp file
    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(source.as_bytes()).unwrap();
    let temp_path = temp_file.path().to_path_buf();

    // Also analyze Simple.java to populate global index
    let simple_path = PathBuf::from("tests/fixtures/Simple.java");

    let mut resolver = TypeResolver::new();

    if simple_path.exists() {
        resolver.analyze_file(&simple_path).unwrap();
    }

    // Analyze the temp file
    let result = resolver.analyze_file(&temp_path);
    assert!(result.is_ok());

    // Build global index
    resolver.build_global_index();

    // Add java.util.List to the global index manually for this test
    resolver.global_type_index
        .entry("List".to_string())
        .or_insert_with(Vec::new)
        .push("java.util.List".to_string());

    // Try to resolve "List" via wildcard import
    let resolved = resolver.resolve_type_name("List", &temp_path);
    assert_eq!(resolved, Some("java.util.List".to_string()));
}

#[test]
fn test_multiple_classes_same_simple_name() {
    // Test that global index can handle multiple classes with same simple name
    let mut resolver = TypeResolver::new();

    // Manually create two FileInfos with classes named "List"
    use java_analyzer_provider::java_graph::type_resolver::{FileInfo, ClassInfo};
    use std::collections::HashMap;

    let file1 = PathBuf::from("test1.java");
    let mut classes1 = HashMap::new();
    classes1.insert(
        "List".to_string(),
        ClassInfo {
            simple_name: "List".to_string(),
            fqdn: "com.custom.List".to_string(),
            extends: None,
            implements: vec![],
            methods: vec![],
            fields: vec![],
            is_interface: false,
            is_enum: false,
            position: java_analyzer_provider::java_graph::type_resolver::SourcePosition::unknown(),
        },
    );

    let file_info1 = FileInfo {
        file_path: file1.clone(),
        package_name: Some("com.custom".to_string()),
        explicit_imports: HashMap::new(),
        wildcard_imports: vec![],
        classes: classes1,
        method_calls: vec![],
        constructor_calls: vec![],
        annotations: vec![],
        variables: vec![],
    };

    let file2 = PathBuf::from("test2.java");
    let mut classes2 = HashMap::new();
    classes2.insert(
        "List".to_string(),
        ClassInfo {
            simple_name: "List".to_string(),
            fqdn: "java.util.List".to_string(),
            extends: None,
            implements: vec![],
            methods: vec![],
            fields: vec![],
            is_interface: true,
            is_enum: false,
            position: java_analyzer_provider::java_graph::type_resolver::SourcePosition::unknown(),
        },
    );

    let file_info2 = FileInfo {
        file_path: file2.clone(),
        package_name: Some("java.util".to_string()),
        explicit_imports: HashMap::new(),
        wildcard_imports: vec![],
        classes: classes2,
        method_calls: vec![],
        constructor_calls: vec![],
        annotations: vec![],
        variables: vec![],
    };

    resolver.file_infos.insert(file1, file_info1);
    resolver.file_infos.insert(file2, file_info2);

    resolver.build_global_index();

    // Global index should have both FQDNs for "List"
    let list_fqdns = resolver.global_type_index.get("List").unwrap();
    assert_eq!(list_fqdns.len(), 2);
    assert!(list_fqdns.contains(&"com.custom.List".to_string()));
    assert!(list_fqdns.contains(&"java.util.List".to_string()));
}

#[test]
fn test_constructor_extraction() {
    let fixture_path = PathBuf::from("tests/fixtures/Simple.java");
    if !fixture_path.exists() {
        eprintln!("Skipping test - fixture not found");
        return;
    }

    let mut resolver = TypeResolver::new();
    resolver.analyze_file(&fixture_path).unwrap();

    let file_info = resolver.file_infos.get(&fixture_path).unwrap();
    let class_info = file_info.classes.get("Simple").unwrap();

    // Find constructor (named "Simple")
    let constructor = class_info.methods.iter().find(|m| m.name == "Simple");
    assert!(constructor.is_some());

    let ctor = constructor.unwrap();
    assert_eq!(ctor.parameters.len(), 2);
    assert_eq!(ctor.return_type, ""); // Constructors have no return type
}

#[test]
fn test_interface_detection() {
    // Create a simple interface source
    let source = r#"
        package com.test;

        public interface MyInterface {
            void doSomething();
        }
    "#;

    use java_analyzer_provider::java_graph::language_config;
    use tempfile::NamedTempFile;
    use std::io::Write;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(source.as_bytes()).unwrap();
    let temp_path = temp_file.path().to_path_buf();

    let mut resolver = TypeResolver::new();
    resolver.analyze_file(&temp_path).unwrap();

    let file_info = resolver.file_infos.get(&temp_path).unwrap();
    let interface_info = file_info.classes.get("MyInterface").unwrap();

    assert!(interface_info.is_interface);
    assert!(!interface_info.is_enum);
}

#[test]
fn test_enum_detection() {
    // Create a simple enum source
    let source = r#"
        package com.test;

        public enum Status {
            ACTIVE,
            INACTIVE,
            PENDING
        }
    "#;

    use java_analyzer_provider::java_graph::language_config;
    use tempfile::NamedTempFile;
    use std::io::Write;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(source.as_bytes()).unwrap();
    let temp_path = temp_file.path().to_path_buf();

    let mut resolver = TypeResolver::new();
    resolver.analyze_file(&temp_path).unwrap();

    let file_info = resolver.file_infos.get(&temp_path).unwrap();
    let enum_info = file_info.classes.get("Status").unwrap();

    assert!(!enum_info.is_interface);
    assert!(enum_info.is_enum);
}
