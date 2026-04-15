// Test Maven parser directly

use java_analyzer_provider::buildtool::maven::MavenPom;
use std::path::PathBuf;

#[test]
fn test_parse_pom_directly() {
    let pom_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
    <modelVersion>4.0.0</modelVersion>
    <groupId>com.example</groupId>
    <artifactId>test-app</artifactId>
    <version>1.0.0</version>
    <packaging>jar</packaging>

    <dependencies>
        <dependency>
            <groupId>junit</groupId>
            <artifactId>junit</artifactId>
            <version>4.13.2</version>
            <scope>test</scope>
        </dependency>
        <dependency>
            <groupId>org.apache.commons</groupId>
            <artifactId>commons-lang3</artifactId>
            <version>3.12.0</version>
        </dependency>
    </dependencies>
</project>"#;

    let pom = MavenPom::parse_from_string(pom_content, PathBuf::from("pom.xml")).unwrap();

    println!("Group ID: {:?}", pom.group_id);
    println!("Artifact ID: {:?}", pom.artifact_id);
    println!("Version: {:?}", pom.version);
    println!("Number of dependencies: {}", pom.dependencies.len());

    for dep in &pom.dependencies {
        println!("Dependency: {}:{}", dep.group_id, dep.artifact_id);
    }

    assert_eq!(pom.dependencies.len(), 2, "Should have 2 dependencies");
    assert_eq!(pom.dependencies[0].group_id, "junit");
    assert_eq!(pom.dependencies[0].artifact_id, "junit");
    assert_eq!(pom.dependencies[1].group_id, "org.apache.commons");
    assert_eq!(pom.dependencies[1].artifact_id, "commons-lang3");
}
