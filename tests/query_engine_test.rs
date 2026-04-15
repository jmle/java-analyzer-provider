use java_analyzer_provider::java_graph::{
    query::{LocationType, Pattern, QueryEngine, ReferencedQuery},
    type_resolver::TypeResolver,
    loader,
};
use std::path::PathBuf;

fn create_test_query_engine(sources: Vec<(&str, &str)>) -> QueryEngine {
    use tempfile::NamedTempFile;
    use std::io::Write;

    let mut temp_files = Vec::new();
    let mut paths = Vec::new();

    // Create temporary files
    for (_name, source) in sources {
        let mut file = NamedTempFile::new().unwrap();
        file.write_all(source.as_bytes()).unwrap();
        let path = file.path().to_path_buf();
        paths.push(path.clone());
        temp_files.push(file); // Keep file alive
    }

    // Build TypeResolver
    let mut resolver = TypeResolver::new();
    for path in &paths {
        resolver.analyze_file(path).unwrap();
    }
    resolver.build_global_index();
    resolver.build_inheritance_maps();

    // Build StackGraph
    let path_refs: Vec<&std::path::Path> = paths.iter().map(|p| p.as_path()).collect();
    let graph = loader::build_graph_for_files(&path_refs).unwrap();

    // Keep temp_files alive by moving into a Box that lives as long as needed
    std::mem::forget(temp_files);

    QueryEngine::new(graph, resolver)
}

#[test]
fn test_query_classes() {
    let source = r#"
        package com.example;

        public class MyClass {
            private int value;
        }

        class AnotherClass {
        }
    "#;

    let engine = create_test_query_engine(vec![("test.java", source)]);

    let query = ReferencedQuery {
        pattern: Pattern::from_string("MyClass").unwrap(),
        location: LocationType::Class,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].symbol, "MyClass");
    assert_eq!(results[0].fqdn, Some("com.example.MyClass".to_string()));
}

#[test]
fn test_query_classes_wildcard() {
    let source = r#"
        package com.example.services;

        public class UserService {
        }

        public class OrderService {
        }

        public class ProductController {
        }
    "#;

    let engine = create_test_query_engine(vec![("test.java", source)]);

    let query = ReferencedQuery {
        pattern: Pattern::from_string("*Service").unwrap(),
        location: LocationType::Class,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|r| r.symbol == "UserService"));
    assert!(results.iter().any(|r| r.symbol == "OrderService"));
}

#[test]
fn test_query_types() {
    let source = r#"
        package com.example;

        public class MyClass {
        }

        public interface MyInterface {
        }

        public enum Status {
            ACTIVE, INACTIVE
        }
    "#;

    let engine = create_test_query_engine(vec![("test.java", source)]);

    let query = ReferencedQuery {
        pattern: Pattern::from_string("com.example.*").unwrap(),
        location: LocationType::Type,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 3); // class, interface, enum
    assert!(results.iter().any(|r| r.symbol == "MyClass"));
    assert!(results.iter().any(|r| r.symbol == "MyInterface"));
    assert!(results.iter().any(|r| r.symbol == "Status"));
}

#[test]
fn test_query_methods() {
    let source = r#"
        package com.example;

        public class MyClass {
            public void doSomething() {}
            private int getValue() { return 0; }
            public void setName(String name) {}
        }
    "#;

    let engine = create_test_query_engine(vec![("test.java", source)]);

    let query = ReferencedQuery {
        pattern: Pattern::from_string("*Value").unwrap(),
        location: LocationType::Method,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].symbol, "getValue");
}

#[test]
fn test_query_fields() {
    let source = r#"
        package com.example;

        public class MyClass {
            private int id;
            private String name;
            private double balance;
        }
    "#;

    let engine = create_test_query_engine(vec![("test.java", source)]);

    let query = ReferencedQuery {
        pattern: Pattern::from_string("name").unwrap(),
        location: LocationType::Field,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].symbol, "name");
    assert_eq!(results[0].fqdn, Some("com.example.MyClass.name".to_string()));
}

#[test]
fn test_query_enums() {
    let source = r#"
        package com.example;

        public enum Status {
            ACTIVE, INACTIVE
        }

        public class MyClass {
        }

        public enum Priority {
            HIGH, MEDIUM, LOW
        }
    "#;

    let engine = create_test_query_engine(vec![("test.java", source)]);

    let query = ReferencedQuery {
        pattern: Pattern::from_string("*").unwrap(),
        location: LocationType::Enum,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|r| r.symbol == "Status"));
    assert!(results.iter().any(|r| r.symbol == "Priority"));
}

#[test]
fn test_query_inheritance() {
    let source1 = r#"
        package com.example;

        public class BaseClass {
        }
    "#;

    let source2 = r#"
        package com.example;

        public class ChildClass extends BaseClass {
        }

        public class AnotherChild extends BaseClass {
        }
    "#;

    let engine = create_test_query_engine(vec![
        ("base.java", source1),
        ("child.java", source2),
    ]);

    let query = ReferencedQuery {
        pattern: Pattern::from_string("BaseClass").unwrap(),
        location: LocationType::Inheritance,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|r| r.symbol.contains("ChildClass")));
    assert!(results.iter().any(|r| r.symbol.contains("AnotherChild")));
}

#[test]
fn test_query_implements() {
    let source = r#"
        package com.example;

        public class MyClass implements Runnable, Cloneable {
        }

        public class OtherClass implements Runnable {
        }

        public class ThirdClass {
        }
    "#;

    let engine = create_test_query_engine(vec![("test.java", source)]);

    let query = ReferencedQuery {
        pattern: Pattern::from_string("Runnable").unwrap(),
        location: LocationType::ImplementsType,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 2);
    assert!(results.iter().any(|r| r.symbol.contains("MyClass")));
    assert!(results.iter().any(|r| r.symbol.contains("OtherClass")));
}

#[test]
fn test_query_packages() {
    let source = r#"
        package com.example.services;

        public class MyClass {
        }
    "#;

    let engine = create_test_query_engine(vec![("test.java", source)]);

    let query = ReferencedQuery {
        pattern: Pattern::from_string("com.example.*").unwrap(),
        location: LocationType::Package,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);
    assert_eq!(results[0].symbol, "com.example.services");
}

#[test]
fn test_query_imports() {
    let source = r#"
        package com.example;

        import java.util.List;

        public class MyClass {
            private List items;
        }
    "#;

    let engine = create_test_query_engine(vec![("test.java", source)]);

    let query = ReferencedQuery {
        pattern: Pattern::from_string("java.util.*").unwrap(),
        location: LocationType::Import,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert!(results.len() >= 1);
    assert!(results.iter().any(|r| r.fqdn == Some("java.util.List".to_string())));
}

#[test]
fn test_query_imports_wildcard() {
    let source = r#"
        package com.example;

        import java.io.*;

        public class MyClass {
        }
    "#;

    let engine = create_test_query_engine(vec![("test.java", source)]);

    let query = ReferencedQuery {
        pattern: Pattern::from_string("java.io").unwrap(),
        location: LocationType::Import,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert!(results.len() >= 1);
    assert!(results.iter().any(|r| r.symbol.contains("java.io")));
}

#[test]
fn test_query_return_types() {
    let source = r#"
        package com.example;

        import java.util.List;

        public class MyClass {
            public String getName() { return ""; }
            public int getValue() { return 0; }
            public List getItems() { return null; }
        }
    "#;

    let engine = create_test_query_engine(vec![("test.java", source)]);

    let query = ReferencedQuery {
        pattern: Pattern::from_string("String").unwrap(),
        location: LocationType::ReturnType,
        annotated: None,
        filters: None,
    };

    let results = engine.query(&query).unwrap();
    assert_eq!(results.len(), 1);
    assert!(results[0].symbol.contains("getName"));
}

#[test]
fn test_pattern_literal() {
    let pattern = Pattern::from_string("MyClass").unwrap();
    assert!(pattern.matches("MyClass"));
    assert!(!pattern.matches("MyClassOther"));
    assert!(!pattern.matches("Other"));
}

#[test]
fn test_pattern_wildcard() {
    let pattern = Pattern::from_string("com.example.*").unwrap();
    assert!(pattern.matches("com.example.MyClass"));
    assert!(pattern.matches("com.example.services.UserService"));
    assert!(!pattern.matches("com.other.MyClass"));
}

#[test]
fn test_pattern_regex() {
    let pattern = Pattern::from_string(".*Service$").unwrap();
    assert!(pattern.matches("UserService"));
    assert!(pattern.matches("com.example.OrderService"));
    assert!(!pattern.matches("ServiceImpl"));
}
