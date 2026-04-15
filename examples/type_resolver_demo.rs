use java_analyzer_provider::java_graph::type_resolver::TypeResolver;
use std::path::PathBuf;

fn main() {
    println!("=== TypeResolver Demo ===\n");

    let mut resolver = TypeResolver::new();

    // Analyze test fixtures
    let fixtures = [
        "tests/fixtures/Simple.java",
        "tests/fixtures/InheritanceExample.java",
    ];

    for fixture in &fixtures {
        let path = PathBuf::from(fixture);
        if path.exists() {
            match resolver.analyze_file(&path) {
                Ok(_) => println!("✓ Analyzed: {}", fixture),
                Err(e) => eprintln!("✗ Failed to analyze {}: {}", fixture, e),
            }
        } else {
            println!("⊘ Skipping: {} (not found)", fixture);
        }
    }

    println!("\n=== File Analysis Results ===\n");

    for (path, file_info) in &resolver.file_infos {
        println!("File: {}", path.display());
        println!("  Package: {}", file_info.package_name.as_ref().unwrap_or(&"(default)".to_string()));

        println!("  Explicit Imports:");
        for (simple_name, fqdn) in &file_info.explicit_imports {
            println!("    {} → {}", simple_name, fqdn);
        }

        if !file_info.wildcard_imports.is_empty() {
            println!("  Wildcard Imports:");
            for wildcard in &file_info.wildcard_imports {
                println!("    {}.*", wildcard);
            }
        }

        println!("  Classes:");
        for class_info in file_info.classes.values() {
            println!("    {} (FQDN: {})", class_info.simple_name, class_info.fqdn);

            if let Some(parent) = &class_info.extends {
                println!("      extends: {}", parent);
            }

            if !class_info.implements.is_empty() {
                println!("      implements: {}", class_info.implements.join(", "));
            }

            println!("      Fields: {}", class_info.fields.len());
            for field in &class_info.fields {
                println!("        - {}: {}", field.name, field.type_name);
            }

            println!("      Methods: {}", class_info.methods.len());
            for method in &class_info.methods {
                let params: Vec<String> = method.parameters.iter()
                    .map(|(name, type_name)| format!("{}: {}", name, type_name))
                    .collect();
                println!("        - {}({}) -> {}",
                    method.name,
                    params.join(", "),
                    if method.return_type.is_empty() { "(constructor)" } else { &method.return_type }
                );
            }
        }
        println!();
    }

    // Build global index
    resolver.build_global_index();

    println!("=== Global Type Index ===\n");
    for (simple_name, fqdns) in &resolver.global_type_index {
        println!("{} → {:?}", simple_name, fqdns);
    }

    // Test type resolution
    println!("\n=== Type Resolution Examples ===\n");

    if let Some((path, _)) = resolver.file_infos.iter().next() {
        let test_types = vec![
            "int",           // Primitive
            "String",        // java.lang
            "List",          // Explicit import
            "ArrayList",     // Explicit import
        ];

        for type_name in test_types {
            match resolver.resolve_type_name(type_name, path) {
                Some(resolved) => println!("{} → {}", type_name, resolved),
                None => println!("{} → (unresolved)", type_name),
            }
        }
    }

    println!("\n=== Demo Complete ===");
}
