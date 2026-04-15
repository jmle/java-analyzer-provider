use java_analyzer_provider::java_graph::{
    query::{LocationType, Pattern, QueryEngine, ReferencedQuery},
    type_resolver::TypeResolver,
    loader,
};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_simple_constructor_call() {
    let source = r#"package com.example;

public class MyClass {
    public void example() {
        User user = new User();
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
        pattern: Pattern::from_string("User").unwrap(),
        location: LocationType::ConstructorCall,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert!(result.symbol.contains("User"));
    assert_eq!(result.line_number, 5); // new User() is on line 5

    std::mem::forget(file);
}

#[test]
fn test_constructor_with_arguments() {
    let source = r#"package com.example;

public class MyClass {
    public void example() {
        User user = new User("John", 30);
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
        pattern: Pattern::from_string("User").unwrap(),
        location: LocationType::ConstructorCall,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].line_number, 5);

    std::mem::forget(file);
}

#[test]
fn test_generic_constructor() {
    let source = r#"package com.example;

import java.util.ArrayList;

public class MyClass {
    public void example() {
        ArrayList<String> list = new ArrayList<>();
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
        pattern: Pattern::from_string("ArrayList").unwrap(),
        location: LocationType::ConstructorCall,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert!(result.symbol.contains("ArrayList"));
    assert_eq!(result.line_number, 7);

    std::mem::forget(file);
}

#[test]
fn test_multiple_constructors() {
    let source = r#"package com.example;

public class MyClass {
    public void example() {
        User user = new User();
        Product product = new Product();
        Order order = new Order();
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
        location: LocationType::ConstructorCall,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 3);

    // Verify different line numbers
    let lines: Vec<usize> = results.iter().map(|r| r.line_number).collect();
    assert!(lines.contains(&5));
    assert!(lines.contains(&6));
    assert!(lines.contains(&7));

    std::mem::forget(file);
}

#[test]
fn test_constructor_in_return_statement() {
    let source = r#"package com.example;

public class MyClass {
    public User createUser() {
        return new User();
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
        pattern: Pattern::from_string("User").unwrap(),
        location: LocationType::ConstructorCall,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].line_number, 5);

    std::mem::forget(file);
}

#[test]
fn test_constructor_as_argument() {
    let source = r#"package com.example;

public class MyClass {
    public void example() {
        process(new User());
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
        pattern: Pattern::from_string("User").unwrap(),
        location: LocationType::ConstructorCall,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].line_number, 5);

    std::mem::forget(file);
}

#[test]
fn test_constructor_position_not_zero() {
    let source = r#"package com.example;

public class MyClass {
    public void example() {
        User first = new User();
        User second = new User();
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
        location: LocationType::ConstructorCall,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert!(results.len() >= 2);

    for result in &results {
        assert_ne!(result.line_number, 0, "Constructor position should not be 0");
    }

    std::mem::forget(file);
}

#[test]
fn test_constructor_pattern_matching() {
    let source = r#"package com.example;

public class MyClass {
    public void example() {
        UserService service = new UserService();
        ProductService product = new ProductService();
        OrderController controller = new OrderController();
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
        pattern: Pattern::from_string("*Service").unwrap(),
        location: LocationType::ConstructorCall,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 2); // UserService and ProductService

    std::mem::forget(file);
}

#[test]
fn test_array_creation() {
    let source = r#"package com.example;

public class MyClass {
    public void example() {
        User[] users = new User[10];
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
        pattern: Pattern::from_string("User").unwrap(),
        location: LocationType::ConstructorCall,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    // Array creation might or might not be captured depending on AST structure
    // This test is mainly to ensure we don't crash on array creation

    std::mem::forget(file);
}
