// Integration tests for the gRPC service

use java_analyzer_provider::analyzer_service::{
    provider_service_server::{ProviderService, ProviderServiceServer},
    Config, EvaluateRequest,
};
use java_analyzer_provider::provider::java::JavaProvider;
use std::io::Write;
use tempfile::TempDir;
use tonic::Request;

#[tokio::test]
async fn test_capabilities() {
    let provider = JavaProvider::new();
    let request = Request::new(());

    let response = provider.capabilities(request).await.unwrap();
    let capabilities = response.into_inner();

    assert!(capabilities.capabilities.len() > 0);
    assert!(capabilities.capabilities.iter().any(|c| c.name == "referenced"));
}

#[tokio::test]
async fn test_init_with_valid_path() {
    // Create a temporary directory with a Java file
    let temp_dir = TempDir::new().unwrap();
    let java_file_path = temp_dir.path().join("Test.java");

    let source = r#"
package com.example;

public class Test {
    private int value;
}
"#;

    std::fs::write(&java_file_path, source).unwrap();

    let provider = JavaProvider::new();
    let config = Config {
        location: temp_dir.path().to_str().unwrap().to_string(),
        dependency_path: String::new(),
        analysis_mode: String::new(),
        provider_specific_config: None,
        proxy: None,
        language_server_pipe: String::new(),
        initialized: false,
    };

    let request = Request::new(config);
    let response = provider.init(request).await.unwrap();
    let init_response = response.into_inner();

    assert!(init_response.successful, "Init should succeed: {}", init_response.error);
    assert!(init_response.error.is_empty());
}

#[tokio::test]
async fn test_init_with_invalid_path() {
    let provider = JavaProvider::new();
    let config = Config {
        location: "/nonexistent/path".to_string(),
        dependency_path: String::new(),
        analysis_mode: String::new(),
        provider_specific_config: None,
        proxy: None,
        language_server_pipe: String::new(),
        initialized: false,
    };

    let request = Request::new(config);
    let response = provider.init(request).await.unwrap();
    let init_response = response.into_inner();

    assert!(!init_response.successful);
    assert!(!init_response.error.is_empty());
}

#[tokio::test]
async fn test_evaluate_simple_query() {
    // Create a temporary directory with a Java file
    let temp_dir = TempDir::new().unwrap();
    let java_file_path = temp_dir.path().join("Test.java");

    let source = r#"
package com.example;

import java.util.List;

public class Test {
    private List items;
}
"#;

    std::fs::write(&java_file_path, source).unwrap();

    let provider = JavaProvider::new();

    // Initialize first
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

    // Now evaluate a query
    let condition_json = r#"{"referenced":{"pattern":"List","location":"import"}}"#;

    let evaluate_request = EvaluateRequest {
        cap: "referenced".to_string(),
        condition_info: condition_json.to_string(),
        id: 1,
    };

    let response = provider.evaluate(Request::new(evaluate_request)).await.unwrap();
    let evaluate_response = response.into_inner();

    assert!(evaluate_response.successful, "Evaluate should succeed: {}", evaluate_response.error);
    assert!(evaluate_response.response.is_some());

    let provider_response = evaluate_response.response.unwrap();
    assert!(provider_response.matched);
    assert!(provider_response.incident_contexts.len() > 0);
}

#[tokio::test]
async fn test_evaluate_method_call_query() {
    // Create a temporary directory with a Java file
    let temp_dir = TempDir::new().unwrap();
    let java_file_path = temp_dir.path().join("Test.java");

    let source = r#"
package com.example;

public class Test {
    public void example() {
        System.out.println("Hello");
    }
}
"#;

    std::fs::write(&java_file_path, source).unwrap();

    let provider = JavaProvider::new();

    // Initialize first
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

    // Query for println method calls
    let condition_json = r#"{"referenced":{"pattern":"println","location":"method_call"}}"#;

    let evaluate_request = EvaluateRequest {
        cap: "referenced".to_string(),
        condition_info: condition_json.to_string(),
        id: 1,
    };

    let response = provider.evaluate(Request::new(evaluate_request)).await.unwrap();
    let evaluate_response = response.into_inner();

    assert!(evaluate_response.successful);
    assert!(evaluate_response.response.is_some());

    let provider_response = evaluate_response.response.unwrap();
    assert!(provider_response.matched);
    assert_eq!(provider_response.incident_contexts.len(), 1);

    let incident = &provider_response.incident_contexts[0];
    assert!(incident.file_uri.contains("Test.java"));
    assert!(incident.line_number.is_some());
}

#[tokio::test]
async fn test_evaluate_without_init() {
    let provider = JavaProvider::new();

    let condition_json = r#"{"referenced":{"pattern":"List","location":"import"}}"#;

    let evaluate_request = EvaluateRequest {
        cap: "referenced".to_string(),
        condition_info: condition_json.to_string(),
        id: 1,
    };

    let response = provider.evaluate(Request::new(evaluate_request)).await.unwrap();
    let evaluate_response = response.into_inner();

    assert!(!evaluate_response.successful);
    assert!(evaluate_response.error.contains("not initialized"));
}
