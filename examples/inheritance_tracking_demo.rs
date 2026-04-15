use java_analyzer_provider::java_graph::type_resolver::TypeResolver;
use std::path::PathBuf;

fn main() {
    println!("=== Inheritance Tracking Demo ===\n");

    let fixtures = vec![
        "tests/fixtures/InheritanceExample.java",
        "tests/fixtures/AdvancedFeatures.java",
    ];

    let mut resolver = TypeResolver::new();
    let mut analyzed_count = 0;

    // Analyze all files
    for path_str in &fixtures {
        let path = PathBuf::from(path_str);
        if path.exists() {
            match resolver.analyze_file(&path) {
                Ok(_) => {
                    println!("✓ Analyzed: {}", path.display());
                    analyzed_count += 1;
                }
                Err(e) => eprintln!("✗ Failed to analyze {}: {}", path.display(), e),
            }
        }
    }

    if analyzed_count == 0 {
        println!("\n⊘ No files to analyze");
        return;
    }

    // Build indexes and inheritance maps
    resolver.build_global_index();
    resolver.build_inheritance_maps();

    println!("\n=== Inheritance Hierarchy ===\n");
    println!("Total classes: {}", resolver.global_type_index.len());
    println!("Classes with parents: {}", resolver.inheritance_map.len());
    println!("Classes with interfaces: {}", resolver.interface_map.len());

    println!("\n=== Inheritance Details ===\n");

    for (class_fqdn, parent_fqdn) in &resolver.inheritance_map {
        println!("Class: {}", class_fqdn);
        println!("  extends: {}", parent_fqdn);

        // Show all parents (transitive)
        let all_parents = resolver.get_all_parents(class_fqdn);
        if all_parents.len() > 1 {
            println!("  all parents: {}", all_parents.join(" → "));
        }

        // Show interfaces
        if let Some(interfaces) = resolver.interface_map.get(class_fqdn) {
            println!("  implements: {}", interfaces.join(", "));
        }

        // Show all interfaces (including from parents)
        let all_interfaces = resolver.get_all_interfaces(class_fqdn);
        if all_interfaces.len() > interfaces_count(resolver.interface_map.get(class_fqdn)) {
            println!("  all interfaces: {}", all_interfaces.join(", "));
        }

        println!();
    }

    println!("=== Transitive Queries Demo ===\n");

    // Demo transitive queries
    for class_fqdn in resolver.global_type_index.values().flatten() {
        if resolver.get_parent_class(class_fqdn).is_some() {
            println!("Class: {}", class_fqdn);

            // Test extends_class
            let all_parents = resolver.get_all_parents(class_fqdn);
            for parent in &all_parents {
                let simple_name = parent.split('.').last().unwrap_or(parent);
                println!("  ✓ extends {} ({})", simple_name,
                    if resolver.extends_class(class_fqdn, simple_name) { "verified" } else { "FAILED" });
            }

            // Test implements_interface
            let all_interfaces = resolver.get_all_interfaces(class_fqdn);
            for interface in &all_interfaces {
                let simple_name = interface.split('.').last().unwrap_or(interface);
                println!("  ✓ implements {} ({})", simple_name,
                    if resolver.implements_interface(class_fqdn, simple_name) { "verified" } else { "FAILED" });
            }

            println!();
        }
    }

    println!("=== Summary ===");
    println!("\nInheritance tracking features:");
    println!("  ✓ Direct parent resolution (get_parent_class)");
    println!("  ✓ Transitive parent chain (get_all_parents)");
    println!("  ✓ Direct interface resolution (get_interfaces)");
    println!("  ✓ Transitive interface resolution (get_all_interfaces)");
    println!("  ✓ extends_class queries (simple name & FQDN)");
    println!("  ✓ implements_interface queries (simple name & FQDN)");
    println!("\nNext: Task 1.7 - Basic query engine");
}

fn interfaces_count(opt: Option<&Vec<String>>) -> usize {
    opt.map(|v| v.len()).unwrap_or(0)
}
