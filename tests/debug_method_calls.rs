use java_analyzer_provider::java_graph::type_resolver::TypeResolver;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn debug_method_call_extraction() {
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

    // Debug: Print what method calls were extracted
    if let Some(file_info) = resolver.file_infos.get(&path) {
        println!("Extracted {} method calls:", file_info.method_calls.len());
        for call in &file_info.method_calls {
            println!("  - Method: {}", call.method_name);
            println!("    Receiver: {:?}", call.receiver_type);
            println!("    Position: line {}, col {}", call.position.line, call.position.column);
        }
    }

    assert_eq!(resolver.file_infos.get(&path).unwrap().method_calls.len(), 1);

    std::mem::forget(file);
}
