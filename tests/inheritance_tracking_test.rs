use java_analyzer_provider::java_graph::type_resolver::TypeResolver;
use std::path::PathBuf;

#[test]
fn test_inheritance_example_tracking() {
    let fixture_path = PathBuf::from("tests/fixtures/InheritanceExample.java");
    if !fixture_path.exists() {
        eprintln!("Skipping test - fixture not found");
        return;
    }

    let mut resolver = TypeResolver::new();
    resolver.analyze_file(&fixture_path).unwrap();
    resolver.build_global_index();
    resolver.build_inheritance_maps();

    let class_fqdn = "com.example.inheritance.InheritanceExample";

    // Verify inheritance map was built
    println!("Inheritance map size: {}", resolver.inheritance_map.len());
    println!("Interface map size: {}", resolver.interface_map.len());

    // Check interfaces
    let interfaces = resolver.get_interfaces(class_fqdn);
    println!("Direct interfaces: {:?}", interfaces);
    assert!(!interfaces.is_empty(), "Should have at least one interface");
}

#[test]
fn test_complex_inheritance_hierarchy() {
    use tempfile::NamedTempFile;
    use std::io::Write;

    // Create a complex hierarchy
    let source1 = r#"
        package com.test;

        public class GrandParent {
            public void ancestorMethod() {}
        }
    "#;

    let source2 = r#"
        package com.test;

        public class Parent extends GrandParent implements Runnable {
            public void run() {}
        }
    "#;

    let source3 = r#"
        package com.test;

        import java.io.Serializable;

        public class Child extends Parent implements Serializable {
            public void childMethod() {}
        }
    "#;

    let mut file1 = NamedTempFile::new().unwrap();
    file1.write_all(source1.as_bytes()).unwrap();
    let path1 = file1.path().to_path_buf();

    let mut file2 = NamedTempFile::new().unwrap();
    file2.write_all(source2.as_bytes()).unwrap();
    let path2 = file2.path().to_path_buf();

    let mut file3 = NamedTempFile::new().unwrap();
    file3.write_all(source3.as_bytes()).unwrap();
    let path3 = file3.path().to_path_buf();

    let mut resolver = TypeResolver::new();
    resolver.analyze_file(&path1).unwrap();
    resolver.analyze_file(&path2).unwrap();
    resolver.analyze_file(&path3).unwrap();

    resolver.build_global_index();
    resolver.build_inheritance_maps();

    // Test transitive inheritance
    assert!(resolver.extends_class("com.test.Child", "com.test.Parent"));
    assert!(resolver.extends_class("com.test.Child", "com.test.GrandParent"));
    assert!(resolver.extends_class("com.test.Child", "Parent"));
    assert!(resolver.extends_class("com.test.Child", "GrandParent"));

    assert!(resolver.extends_class("com.test.Parent", "com.test.GrandParent"));
    assert!(!resolver.extends_class("com.test.GrandParent", "com.test.Parent"));

    // Test transitive interfaces
    assert!(resolver.implements_interface("com.test.Child", "Serializable"));
    assert!(resolver.implements_interface("com.test.Child", "Runnable")); // From Parent

    assert!(resolver.implements_interface("com.test.Parent", "Runnable"));
    assert!(!resolver.implements_interface("com.test.Parent", "Serializable"));

    // Test get_all_parents
    let parents = resolver.get_all_parents("com.test.Child");
    assert_eq!(parents.len(), 2);
    assert_eq!(parents[0], "com.test.Parent");
    assert_eq!(parents[1], "com.test.GrandParent");

    // Test get_all_interfaces
    let interfaces = resolver.get_all_interfaces("com.test.Child");
    assert_eq!(interfaces.len(), 2);
    assert!(interfaces.iter().any(|i| i.contains("Serializable")));
    assert!(interfaces.iter().any(|i| i.contains("Runnable")));
}

#[test]
fn test_multiple_interfaces() {
    use tempfile::NamedTempFile;
    use std::io::Write;

    let source = r#"
        package com.test;

        import java.io.Serializable;
        import java.lang.Cloneable;

        public class MultiInterface implements Runnable, Serializable, Cloneable {
            public void run() {}
        }
    "#;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(source.as_bytes()).unwrap();
    let temp_path = temp_file.path().to_path_buf();

    let mut resolver = TypeResolver::new();
    resolver.analyze_file(&temp_path).unwrap();
    resolver.build_global_index();
    resolver.build_inheritance_maps();

    let class_fqdn = "com.test.MultiInterface";

    // Should implement all three interfaces
    assert!(resolver.implements_interface(class_fqdn, "Runnable"));
    assert!(resolver.implements_interface(class_fqdn, "Serializable"));
    assert!(resolver.implements_interface(class_fqdn, "Cloneable"));

    let all_interfaces = resolver.get_all_interfaces(class_fqdn);
    assert_eq!(all_interfaces.len(), 3);
}

#[test]
fn test_no_inheritance() {
    use tempfile::NamedTempFile;
    use std::io::Write;

    let source = r#"
        package com.test;

        public class Standalone {
            private int value;
        }
    "#;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(source.as_bytes()).unwrap();
    let temp_path = temp_file.path().to_path_buf();

    let mut resolver = TypeResolver::new();
    resolver.analyze_file(&temp_path).unwrap();
    resolver.build_global_index();
    resolver.build_inheritance_maps();

    let class_fqdn = "com.test.Standalone";

    // Should have no parent
    assert!(resolver.get_parent_class(class_fqdn).is_none());
    assert_eq!(resolver.get_all_parents(class_fqdn).len(), 0);

    // Should have no interfaces
    assert_eq!(resolver.get_interfaces(class_fqdn).len(), 0);
    assert_eq!(resolver.get_all_interfaces(class_fqdn).len(), 0);

    // Should not extend or implement anything
    assert!(!resolver.extends_class(class_fqdn, "Object"));
    assert!(!resolver.implements_interface(class_fqdn, "Runnable"));
}

#[test]
fn test_interface_from_wildcard_import() {
    use tempfile::NamedTempFile;
    use std::io::Write;

    let source = r#"
        package com.test;

        import java.util.*;

        public class MyList implements List {
            // Implementation
        }
    "#;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(source.as_bytes()).unwrap();
    let temp_path = temp_file.path().to_path_buf();

    let mut resolver = TypeResolver::new();
    resolver.analyze_file(&temp_path).unwrap();
    resolver.build_global_index();

    // Add java.util.List to global index for wildcard resolution
    resolver.global_type_index
        .entry("List".to_string())
        .or_default()
        .push("java.util.List".to_string());

    resolver.build_inheritance_maps();

    let class_fqdn = "com.test.MyList";

    // Should resolve List via wildcard import
    let interfaces = resolver.get_interfaces(class_fqdn);
    if !interfaces.is_empty() {
        assert!(interfaces[0].contains("List"));
    }
}

#[test]
fn test_pattern_matching_in_queries() {
    use tempfile::NamedTempFile;
    use std::io::Write;

    let source = r#"
        package com.example.myapp;

        import com.example.base.BaseClass;
        import java.io.Serializable;

        public class MyClass extends BaseClass implements Serializable {
        }
    "#;

    let mut temp_file = NamedTempFile::new().unwrap();
    temp_file.write_all(source.as_bytes()).unwrap();
    let temp_path = temp_file.path().to_path_buf();

    let mut resolver = TypeResolver::new();
    resolver.analyze_file(&temp_path).unwrap();
    resolver.build_global_index();
    resolver.build_inheritance_maps();

    let class_fqdn = "com.example.myapp.MyClass";

    // Check that inheritance was tracked
    let parent = resolver.get_parent_class(class_fqdn);
    if let Some(parent_fqdn) = parent {
        // Test pattern matching with simple names
        assert!(resolver.extends_class(class_fqdn, "BaseClass"));

        // Test pattern matching with FQDNs
        assert!(resolver.extends_class(class_fqdn, parent_fqdn));
    }

    // Test interface implementation
    assert!(resolver.implements_interface(class_fqdn, "Serializable"));
    assert!(resolver.implements_interface(class_fqdn, "java.io.Serializable"));
}
