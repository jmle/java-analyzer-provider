use java_analyzer_provider::java_graph::type_resolver::TypeResolver;
use std::io::Write;
use tempfile::NamedTempFile;

#[test]
fn debug_constructor_call_extraction() {
    let source = r#"package com.example;

import java.util.ArrayList;

public class MyClass {
    public void example() {
        User user = new User("John");
        ArrayList<String> list = new ArrayList<>();
    }
}
"#;

    let mut file = NamedTempFile::new().unwrap();
    file.write_all(source.as_bytes()).unwrap();
    let path = file.path().to_path_buf();

    let mut resolver = TypeResolver::new();
    resolver.analyze_file(&path).unwrap();

    // Debug: Print what constructor calls were extracted
    if let Some(file_info) = resolver.file_infos.get(&path) {
        println!("Extracted {} constructor calls:", file_info.constructor_calls.len());
        for call in &file_info.constructor_calls {
            println!("  - Type: {}", call.type_name);
            println!("    Position: line {}, col {}", call.position.line, call.position.column);
        }
    }

    assert_eq!(resolver.file_infos.get(&path).unwrap().constructor_calls.len(), 2);

    std::mem::forget(file);
}

#[test]
fn debug_constructor_type_resolution() {
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

    // Debug: Print type resolution
    if let Some(file_info) = resolver.file_infos.get(&path) {
        for call in &file_info.constructor_calls {
            let resolved = resolver.resolve_type_name(&call.type_name, &path);
            println!("Type: {} → Resolved: {:?}", call.type_name, resolved);
        }
    }

    std::mem::forget(file);
}
