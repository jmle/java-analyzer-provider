// Integration tests for Maven dependency resolution

use java_analyzer_provider::analyzer_service::{
    provider_service_server::ProviderService,
    Config, ServiceRequest,
};
use java_analyzer_provider::provider::java::JavaProvider;
use std::io::Write;
use tempfile::TempDir;
use tonic::Request;

#[tokio::test]
async fn test_maven_dependency_resolution() {
    // Initialize tracing for debugging
    let _ = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .with_test_writer()
        .try_init();

    // Create a temporary directory with a pom.xml
    let temp_dir = TempDir::new().unwrap();
    let pom_path = temp_dir.path().join("pom.xml");

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

    std::fs::write(&pom_path, pom_content).unwrap();

    let provider = JavaProvider::new();

    // Initialize
    let config = Config {
        location: temp_dir.path().to_str().unwrap().to_string(),
        dependency_path: String::new(),
        analysis_mode: String::new(),
        provider_specific_config: None,
        proxy: None,
        language_server_pipe: String::new(),
        initialized: false,
    };

    let init_response = provider.init(Request::new(config)).await.unwrap();
    assert!(init_response.into_inner().successful);

    // Get dependencies
    let dep_request = ServiceRequest { id: 1 };
    let response = provider.get_dependencies(Request::new(dep_request)).await.unwrap();
    let dep_response = response.into_inner();

    assert!(dep_response.successful, "GetDependencies should succeed: {}", dep_response.error);
    assert_eq!(dep_response.file_dep.len(), 1, "Should have dependencies for one pom.xml");

    let file_dep = &dep_response.file_dep[0];
    assert!(file_dep.file_uri.contains("pom.xml"));

    if let Some(ref dep_list) = file_dep.list {
        println!("Dependency list has {} dependencies", dep_list.deps.len());
        for dep in &dep_list.deps {
            println!("Found dependency: {} version {}", dep.name, dep.version);
        }
        assert_eq!(dep_list.deps.len(), 2, "Should have 2 dependencies");

        // Check junit dependency
        let junit_dep = dep_list.deps.iter().find(|d| d.name == "junit");
        assert!(junit_dep.is_some(), "Should have junit dependency");
        let junit = junit_dep.unwrap();
        assert_eq!(junit.version, "4.13.2");
        assert!(junit.resolved_identifier.contains("junit:junit:4.13.2"));

        // Check commons-lang3 dependency
        let commons_dep = dep_list.deps.iter().find(|d| d.name == "commons-lang3");
        assert!(commons_dep.is_some(), "Should have commons-lang3 dependency");
        let commons = commons_dep.unwrap();
        assert_eq!(commons.version, "3.12.0");
    } else {
        panic!("Dependency list should not be None");
    }
}

#[tokio::test]
async fn test_maven_without_pom() {
    // Create a temporary directory WITHOUT a pom.xml
    let temp_dir = TempDir::new().unwrap();

    let provider = JavaProvider::new();

    // Initialize
    let config = Config {
        location: temp_dir.path().to_str().unwrap().to_string(),
        dependency_path: String::new(),
        analysis_mode: String::new(),
        provider_specific_config: None,
        proxy: None,
        language_server_pipe: String::new(),
        initialized: false,
    };

    let init_response = provider.init(Request::new(config)).await.unwrap();
    assert!(init_response.into_inner().successful);

    // Get dependencies (should succeed but return no dependencies)
    let dep_request = ServiceRequest { id: 1 };
    let response = provider.get_dependencies(Request::new(dep_request)).await.unwrap();
    let dep_response = response.into_inner();

    assert!(dep_response.successful);
    assert_eq!(dep_response.file_dep.len(), 0, "Should have no dependencies without pom.xml");
}

#[tokio::test]
async fn test_maven_with_parent() {
    // Create a temporary directory with a pom.xml that has a parent
    let temp_dir = TempDir::new().unwrap();
    let pom_path = temp_dir.path().join("pom.xml");

    let pom_content = r#"<?xml version="1.0" encoding="UTF-8"?>
<project xmlns="http://maven.apache.org/POM/4.0.0">
    <modelVersion>4.0.0</modelVersion>

    <parent>
        <groupId>org.springframework.boot</groupId>
        <artifactId>spring-boot-starter-parent</artifactId>
        <version>2.7.0</version>
    </parent>

    <groupId>com.example</groupId>
    <artifactId>my-spring-app</artifactId>
    <version>1.0.0</version>

    <dependencies>
        <dependency>
            <groupId>org.springframework.boot</groupId>
            <artifactId>spring-boot-starter-web</artifactId>
        </dependency>
    </dependencies>
</project>"#;

    std::fs::write(&pom_path, pom_content).unwrap();

    let provider = JavaProvider::new();

    // Initialize
    let config = Config {
        location: temp_dir.path().to_str().unwrap().to_string(),
        dependency_path: String::new(),
        analysis_mode: String::new(),
        provider_specific_config: None,
        proxy: None,
        language_server_pipe: String::new(),
        initialized: false,
    };

    let init_response = provider.init(Request::new(config)).await.unwrap();
    assert!(init_response.into_inner().successful);

    // Get dependencies
    let dep_request = ServiceRequest { id: 1 };
    let response = provider.get_dependencies(Request::new(dep_request)).await.unwrap();
    let dep_response = response.into_inner();

    assert!(dep_response.successful);
    assert_eq!(dep_response.file_dep.len(), 1);

    let file_dep = &dep_response.file_dep[0];
    if let Some(ref dep_list) = file_dep.list {
        // Should have at least the spring-boot-starter-web dependency
        assert!(dep_list.deps.len() >= 1);

        let spring_dep = dep_list.deps.iter().find(|d| d.name == "spring-boot-starter-web");
        assert!(spring_dep.is_some(), "Should have spring-boot-starter-web dependency");
    }
}

#[tokio::test]
async fn test_dependency_resolution_before_init() {
    let provider = JavaProvider::new();

    // Try to get dependencies without init
    let dep_request = ServiceRequest { id: 1 };
    let response = provider.get_dependencies(Request::new(dep_request)).await.unwrap();
    let dep_response = response.into_inner();

    assert!(!dep_response.successful);
    assert!(dep_response.error.contains("not initialized"));
}
