// Integration tests for Gradle dependency resolution

use java_analyzer_provider::analyzer_service::{
    provider_service_server::ProviderService,
    Config, ServiceRequest,
};
use java_analyzer_provider::provider::java::JavaProvider;
use tempfile::TempDir;
use tonic::Request;

#[tokio::test]
async fn test_gradle_dependency_resolution() {
    // Create a temporary directory with a build.gradle
    let temp_dir = TempDir::new().unwrap();
    let build_path = temp_dir.path().join("build.gradle");

    let build_content = r#"
plugins {
    id 'java'
}

group 'com.example'
version '1.0.0'

dependencies {
    implementation 'org.springframework.boot:spring-boot-starter-web:2.7.0'
    testImplementation 'junit:junit:4.13.2'
    compileOnly 'org.projectlombok:lombok:1.18.24'
}
"#;

    std::fs::write(&build_path, build_content).unwrap();

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
    assert_eq!(dep_response.file_dep.len(), 1, "Should have dependencies for one build.gradle");

    let file_dep = &dep_response.file_dep[0];
    assert!(file_dep.file_uri.contains("build.gradle"));

    if let Some(ref dep_list) = file_dep.list {
        assert_eq!(dep_list.deps.len(), 3, "Should have 3 dependencies");

        // Check spring dependency
        let spring_dep = dep_list.deps.iter().find(|d| d.name == "spring-boot-starter-web");
        assert!(spring_dep.is_some(), "Should have spring-boot-starter-web dependency");
        let spring = spring_dep.unwrap();
        assert_eq!(spring.version, "2.7.0");

        // Check junit dependency
        let junit_dep = dep_list.deps.iter().find(|d| d.name == "junit");
        assert!(junit_dep.is_some(), "Should have junit dependency");
        let junit = junit_dep.unwrap();
        assert_eq!(junit.version, "4.13.2");

        // Check lombok dependency
        let lombok_dep = dep_list.deps.iter().find(|d| d.name == "lombok");
        assert!(lombok_dep.is_some(), "Should have lombok dependency");
    } else {
        panic!("Dependency list should not be None");
    }
}

#[tokio::test]
async fn test_gradle_kotlin_dsl() {
    // Create a temporary directory with a build.gradle.kts
    let temp_dir = TempDir::new().unwrap();
    let build_path = temp_dir.path().join("build.gradle.kts");

    let build_content = r#"
plugins {
    java
}

group = "com.example"
version = "1.0.0"

dependencies {
    implementation("com.google.guava:guava:31.1-jre")
    testImplementation("org.junit.jupiter:junit-jupiter:5.9.0")
}
"#;

    std::fs::write(&build_path, build_content).unwrap();

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
    assert!(file_dep.file_uri.contains("build.gradle.kts"));

    if let Some(ref dep_list) = file_dep.list {
        assert!(dep_list.deps.len() >= 2);

        let guava_dep = dep_list.deps.iter().find(|d| d.name == "guava");
        assert!(guava_dep.is_some(), "Should have guava dependency");

        let junit_dep = dep_list.deps.iter().find(|d| d.name == "junit-jupiter");
        assert!(junit_dep.is_some(), "Should have junit-jupiter dependency");
    }
}

#[tokio::test]
async fn test_gradle_without_build_file() {
    // Create a temporary directory WITHOUT a build.gradle
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
    assert_eq!(dep_response.file_dep.len(), 0, "Should have no dependencies without build file");
}
