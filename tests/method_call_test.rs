use java_analyzer_provider::java_graph::{
    query::{LocationType, Pattern, QueryEngine, ReferencedQuery},
    type_resolver::TypeResolver,
    loader,
};
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn test_simple_method_call() {
    let source = r#"package com.example;

public class MyClass {
    public void example() {
        doSomething();
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
        pattern: Pattern::from_string("doSomething").unwrap(),
        location: LocationType::MethodCall,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert_eq!(result.symbol, "doSomething");
    assert_eq!(result.line_number, 5); // doSomething() is on line 5
    assert!(result.column >= 0);

    std::mem::forget(file);
}

#[test]
fn test_method_call_with_receiver() {
    let source = r#"package com.example;

public class MyClass {
    public void example() {
        obj.doSomething();
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
        pattern: Pattern::from_string("doSomething").unwrap(),
        location: LocationType::MethodCall,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert!(result.symbol.contains("doSomething"));
    // Symbol should be "obj.doSomething" or similar
    assert_eq!(result.line_number, 5);

    std::mem::forget(file);
}

#[test]
fn test_multiple_method_calls() {
    let source = r#"package com.example;

public class MyClass {
    public void example() {
        method1();
        method2();
        method3();
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
        pattern: Pattern::from_string("method*").unwrap(),
        location: LocationType::MethodCall,
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
fn test_chained_method_calls() {
    let source = r#"package com.example;

public class MyClass {
    public void example() {
        obj.method1().method2().method3();
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
        pattern: Pattern::from_string("method*").unwrap(),
        location: LocationType::MethodCall,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    // Should find all three chained method calls
    assert!(results.len() >= 3);

    std::mem::forget(file);
}

#[test]
fn test_system_out_println() {
    let source = r#"package com.example;

public class MyClass {
    public void example() {
        System.out.println("Hello");
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
        pattern: Pattern::from_string("println").unwrap(),
        location: LocationType::MethodCall,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert!(result.symbol.contains("println"));
    assert_eq!(result.line_number, 5);

    std::mem::forget(file);
}

#[test]
fn test_method_call_position_not_zero() {
    let source = r#"package com.example;

public class MyClass {
    public void example() {
        first();
        second();
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
        location: LocationType::MethodCall,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert!(results.len() >= 2);

    for result in &results {
        assert_ne!(result.line_number, 0, "Method call position should not be 0");
    }

    std::mem::forget(file);
}

#[test]
fn test_method_call_with_arguments() {
    let source = r#"package com.example;

public class MyClass {
    public void example() {
        process("arg1", 42, true);
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
        pattern: Pattern::from_string("process").unwrap(),
        location: LocationType::MethodCall,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);

    let result = &results[0];
    assert_eq!(result.symbol, "process");
    assert_eq!(result.line_number, 5);

    std::mem::forget(file);
}
