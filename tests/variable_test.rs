use java_analyzer_provider::java_graph::{
    query::{LocationType, Pattern, QueryEngine, ReferencedQuery},
    type_resolver::TypeResolver,
    loader,
};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_simple_variable_declaration() {
    let source = r#"package com.example;

public class MyClass {
    public void example() {
        int count = 0;
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

    // Query by type
    let query = ReferencedQuery {
        pattern: Pattern::from_string("int").unwrap(),
        location: LocationType::Variable,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert!(result.symbol.contains("count"));
    assert!(result.symbol.contains("int"));
    assert_eq!(result.line_number, 5); // int count = 0; is on line 5

    std::mem::forget(file);
}

#[test]
fn test_variable_declaration_by_name() {
    let source = r#"package com.example;

public class MyClass {
    public void example() {
        String name = "John";
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

    // Query by variable name
    let query = ReferencedQuery {
        pattern: Pattern::from_string("name").unwrap(),
        location: LocationType::Variable,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert!(result.symbol.contains("name"));
    assert!(result.symbol.contains("String"));

    std::mem::forget(file);
}

#[test]
fn test_generic_type_variable() {
    let source = r#"package com.example;

import java.util.List;
import java.util.ArrayList;

public class MyClass {
    public void example() {
        List<String> items = new ArrayList<>();
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

    // Query by type (should match List)
    let query = ReferencedQuery {
        pattern: Pattern::from_string("List").unwrap(),
        location: LocationType::Variable,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert!(result.symbol.contains("items"));
    assert!(result.symbol.contains("List"));
    assert_eq!(result.line_number, 8);

    std::mem::forget(file);
}

#[test]
fn test_multiple_variables() {
    let source = r#"package com.example;

public class MyClass {
    public void example() {
        int x = 1;
        int y = 2;
        String name = "test";
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

    // Query for int variables
    let query = ReferencedQuery {
        pattern: Pattern::from_string("int").unwrap(),
        location: LocationType::Variable,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 2); // x and y

    std::mem::forget(file);
}

#[test]
fn test_variable_in_different_methods() {
    let source = r#"package com.example;

public class MyClass {
    public void methodOne() {
        int value = 1;
    }

    public void methodTwo() {
        int value = 2;
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

    // Query by variable name
    let query = ReferencedQuery {
        pattern: Pattern::from_string("value").unwrap(),
        location: LocationType::Variable,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 2); // One in each method

    // Check that both have method context
    for result in &results {
        assert!(result.symbol.contains("method"));
    }

    std::mem::forget(file);
}

#[test]
fn test_variable_position_not_zero() {
    let source = r#"package com.example;

public class MyClass {
    public void example() {
        int x = 1;
        String name = "test";
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
        location: LocationType::Variable,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert!(results.len() >= 2);

    for result in &results {
        assert_ne!(result.line_number, 0, "Variable position should not be 0");
    }

    std::mem::forget(file);
}

#[test]
fn test_variable_pattern_matching() {
    let source = r#"package com.example;

public class MyClass {
    public void example() {
        int count = 0;
        int total = 0;
        String name = "test";
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

    // Find all variables with wildcard
    let query = ReferencedQuery {
        pattern: Pattern::from_string("*").unwrap(),
        location: LocationType::Variable,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 3);

    std::mem::forget(file);
}

#[test]
fn test_variable_with_resolved_type() {
    let source = r#"package com.example;

import java.util.ArrayList;

public class MyClass {
    public void example() {
        ArrayList<String> items = new ArrayList<>();
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

    // Query by FQDN pattern
    let query = ReferencedQuery {
        pattern: Pattern::from_string("java.util.*").unwrap(),
        location: LocationType::Variable,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert!(result.symbol.contains("items"));
    assert_eq!(result.fqdn, Some("java.util.ArrayList".to_string()));

    std::mem::forget(file);
}

#[test]
fn test_variable_in_constructor() {
    let source = r#"package com.example;

public class MyClass {
    public MyClass() {
        int initialized = 1;
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
        pattern: Pattern::from_string("initialized").unwrap(),
        location: LocationType::Variable,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert!(result.symbol.contains("initialized"));
    // Should have method context (constructor)
    assert!(result.symbol.contains("MyClass"));

    std::mem::forget(file);
}

#[test]
fn test_variable_boolean_type() {
    let source = r#"package com.example;

public class MyClass {
    public void example() {
        boolean isValid = true;
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
        pattern: Pattern::from_string("boolean").unwrap(),
        location: LocationType::Variable,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert!(result.symbol.contains("isValid"));
    assert!(result.symbol.contains("boolean"));

    std::mem::forget(file);
}
