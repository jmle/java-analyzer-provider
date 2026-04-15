use java_analyzer_provider::java_graph::{
    query::{LocationType, Pattern, QueryEngine, ReferencedQuery},
    type_resolver::TypeResolver,
    loader,
};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_class_position() {
    let source = r#"package com.example;

public class MyClass {
    private int value;
}
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(source.as_bytes()).unwrap();
    let path = file.path().to_path_buf();

    let mut resolver = TypeResolver::new();
    resolver.analyze_file(&path).unwrap();
    resolver.build_global_index();

    let graph = loader::build_graph_for_files(&[&path]).unwrap();
    let engine = QueryEngine::new(graph, resolver);

    let query = ReferencedQuery {
        pattern: Pattern::from_string("MyClass").unwrap(),
        location: LocationType::Class,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert_eq!(result.symbol, "MyClass");
    assert_eq!(result.line_number, 3); // "public class MyClass" is on line 3
    // Column 0 is valid (tree-sitter uses 0-based column numbers)

    std::mem::forget(file); // Keep file alive
}

#[test]
fn test_method_position() {
    let source = r#"package com.example;

public class MyClass {
    public void myMethod() {
    }
}
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(source.as_bytes()).unwrap();
    let path = file.path().to_path_buf();

    let mut resolver = TypeResolver::new();
    resolver.analyze_file(&path).unwrap();
    resolver.build_global_index();

    let graph = loader::build_graph_for_files(&[&path]).unwrap();
    let engine = QueryEngine::new(graph, resolver);

    let query = ReferencedQuery {
        pattern: Pattern::from_string("myMethod").unwrap(),
        location: LocationType::Method,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert_eq!(result.symbol, "myMethod");
    assert_eq!(result.line_number, 4); // "public void myMethod()" is on line 4
    // Column numbers are 0-based, so we just check line_number

    std::mem::forget(file);
}

#[test]
fn test_field_position() {
    let source = r#"package com.example;

public class MyClass {
    private int value;
    private String name;
}
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(source.as_bytes()).unwrap();
    let path = file.path().to_path_buf();

    let mut resolver = TypeResolver::new();
    resolver.analyze_file(&path).unwrap();
    resolver.build_global_index();

    let graph = loader::build_graph_for_files(&[&path]).unwrap();
    let engine = QueryEngine::new(graph, resolver);

    let query = ReferencedQuery {
        pattern: Pattern::from_string("name").unwrap(),
        location: LocationType::Field,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert_eq!(result.symbol, "name");
    assert_eq!(result.line_number, 5); // "private String name;" is on line 5
    // Column numbers are 0-based, so we just check line_number

    std::mem::forget(file);
}

#[test]
fn test_multiple_classes_positions() {
    let source = r#"package com.example;

class FirstClass {
}

class SecondClass {
}

class ThirdClass {
}
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(source.as_bytes()).unwrap();
    let path = file.path().to_path_buf();

    let mut resolver = TypeResolver::new();
    resolver.analyze_file(&path).unwrap();
    resolver.build_global_index();

    let graph = loader::build_graph_for_files(&[&path]).unwrap();
    let engine = QueryEngine::new(graph, resolver);

    let query = ReferencedQuery {
        pattern: Pattern::from_string("*Class").unwrap(),
        location: LocationType::Class,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 3);

    // Find each class in results
    let first = results.iter().find(|r| r.symbol == "FirstClass").unwrap();
    let second = results.iter().find(|r| r.symbol == "SecondClass").unwrap();
    let third = results.iter().find(|r| r.symbol == "ThirdClass").unwrap();

    // Verify positions are different and in order
    assert_eq!(first.line_number, 3);
    assert_eq!(second.line_number, 6);
    assert_eq!(third.line_number, 9);

    std::mem::forget(file);
}

#[test]
fn test_position_not_zero() {
    let source = r#"package com.example;

public class MyClass {
    private int value;

    public int getValue() {
        return value;
    }
}
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(source.as_bytes()).unwrap();
    let path = file.path().to_path_buf();

    let mut resolver = TypeResolver::new();
    resolver.analyze_file(&path).unwrap();
    resolver.build_global_index();

    let graph = loader::build_graph_for_files(&[&path]).unwrap();
    let engine = QueryEngine::new(graph, resolver);

    // Query for class
    let query = ReferencedQuery {
        pattern: Pattern::from_string("*").unwrap(),
        location: LocationType::Class,
        annotated: None,
        filters: None,
    };
    let results = engine.query(&query).unwrap();
    for result in &results {
        assert_ne!(result.line_number, 0, "Class position should not be 0");
    }

    // Query for methods
    let query = ReferencedQuery {
        pattern: Pattern::from_string("*").unwrap(),
        location: LocationType::Method,
        annotated: None,
        filters: None,
    };
    let results = engine.query(&query).unwrap();
    for result in &results {
        assert_ne!(result.line_number, 0, "Method position should not be 0");
    }

    // Query for fields
    let query = ReferencedQuery {
        pattern: Pattern::from_string("*").unwrap(),
        location: LocationType::Field,
        annotated: None,
        filters: None,
    };
    let results = engine.query(&query).unwrap();
    for result in &results {
        assert_ne!(result.line_number, 0, "Field position should not be 0");
    }

    std::mem::forget(file);
}
