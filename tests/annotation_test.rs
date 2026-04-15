use java_analyzer_provider::java_graph::{
    query::{LocationType, Pattern, QueryEngine, ReferencedQuery},
    type_resolver::TypeResolver,
    loader,
};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_simple_annotation_on_method() {
    let source = r#"package com.example;

public class MyClass {
    @Override
    public String toString() {
        return "MyClass";
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
        pattern: Pattern::from_string("Override").unwrap(),
        location: LocationType::Annotation,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert!(result.symbol.contains("Override"));
    assert_eq!(result.line_number, 4); // @Override is on line 4

    std::mem::forget(file);
}

#[test]
fn test_annotation_with_parameters() {
    let source = r#"package com.example;

public class MyClass {
    @SuppressWarnings("unused")
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
        pattern: Pattern::from_string("SuppressWarnings").unwrap(),
        location: LocationType::Annotation,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert!(result.symbol.contains("SuppressWarnings"));
    assert_eq!(result.line_number, 4);

    std::mem::forget(file);
}

#[test]
fn test_annotation_on_class() {
    let source = r#"package com.example;

@Deprecated
public class MyClass {
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
        pattern: Pattern::from_string("Deprecated").unwrap(),
        location: LocationType::Annotation,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert!(result.symbol.contains("Deprecated"));
    assert!(result.symbol.contains("class"));
    assert_eq!(result.line_number, 3);

    std::mem::forget(file);
}

#[test]
fn test_multiple_annotations() {
    let source = r#"package com.example;

public class MyClass {
    @Override
    @Deprecated
    public String toString() {
        return "MyClass";
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
        pattern: Pattern::from_string("*").unwrap(),
        location: LocationType::Annotation,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 2); // Override and Deprecated

    std::mem::forget(file);
}

#[test]
fn test_annotation_on_field() {
    let source = r#"package com.example;

public class MyClass {
    @Deprecated
    private int oldField;
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
        pattern: Pattern::from_string("Deprecated").unwrap(),
        location: LocationType::Annotation,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert!(result.symbol.contains("oldField"));
    assert_eq!(result.line_number, 4);

    std::mem::forget(file);
}

#[test]
fn test_annotation_pattern_matching() {
    let source = r#"package com.example;

public class MyClass {
    @Override
    public String toString() {
        return "MyClass";
    }

    @Deprecated
    public void oldMethod() {
    }

    @SuppressWarnings("unused")
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

    // Find all annotations
    let query = ReferencedQuery {
        pattern: Pattern::from_string("*").unwrap(),
        location: LocationType::Annotation,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 3);

    std::mem::forget(file);
}

#[test]
fn test_annotation_position_not_zero() {
    let source = r#"package com.example;

public class MyClass {
    @Override
    public String toString() {
        return "MyClass";
    }

    @Deprecated
    public void oldMethod() {
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
        pattern: Pattern::from_string("*").unwrap(),
        location: LocationType::Annotation,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert!(results.len() >= 2);

    for result in &results {
        assert_ne!(result.line_number, 0, "Annotation position should not be 0");
    }

    std::mem::forget(file);
}

#[test]
fn test_custom_annotation() {
    let source = r#"package com.example;

public class MyClass {
    @MyCustomAnnotation
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
        pattern: Pattern::from_string("MyCustomAnnotation").unwrap(),
        location: LocationType::Annotation,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert!(result.symbol.contains("MyCustomAnnotation"));

    std::mem::forget(file);
}
