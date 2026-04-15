// Main Java provider implementation

use crate::java_graph::{
    type_resolver::TypeResolver,
    query::{QueryEngine, ReferencedQuery, LocationType, Pattern},
    loader,
};
use crate::buildtool::{
    detector::{detect_build_tool, BuildTool},
    maven::{find_pom_files, MavenResolver},
    gradle::{find_gradle_files, GradleResolver},
};
use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use tokio::sync::RwLock;
use tonic::{Request, Response, Status};
use tracing::{debug, info, warn};

// Re-export generated protobuf types
use crate::analyzer_service::{
    provider_service_server::ProviderService,
    provider_code_location_service_server::ProviderCodeLocationService,
    Capability, CapabilitiesResponse, Config, InitResponse, EvaluateRequest,
    EvaluateResponse, ProviderEvaluateResponse, ServiceRequest, DependencyResponse,
    DependencyDagResponse, NotifyFileChangesRequest, NotifyFileChangesResponse,
    PrepareRequest, PrepareResponse, PrepareProgressRequest, ProgressEvent,
    IncidentContext, Position, Location, Dependency, DependencyList, FileDep,
    GetCodeSnipRequest, GetCodeSnipResponse,
};

/// Condition structure matching analyzer-lsp's format
/// The analyzer sends: tags, template, ruleID, and the capability data
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConditionWrapper {
    #[serde(default)]
    pub tags: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub template: HashMap<String, serde_json::Value>,
    #[serde(default, rename = "ruleID")]
    pub rule_id: String,
    pub referenced: ReferencedCondition,
}

#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReferencedCondition {
    pub pattern: String,
    #[serde(default = "default_location")]
    pub location: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub annotated: Option<AnnotatedCondition>,
}

/// Default location type when not specified
fn default_location() -> String {
    "TYPE".to_string()
}

/// Annotation matching condition
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotatedCondition {
    pub pattern: Option<String>,
    #[serde(default)]
    pub elements: Vec<AnnotationElement>,
}

/// Annotation element (name-value pair)
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AnnotationElement {
    pub name: String,
    pub value: String,
}

/// State of the Java provider
pub struct JavaProviderState {
    config: Option<Config>,
    type_resolver: Option<TypeResolver>,
    initialized: bool,
    source_path: Option<PathBuf>,
    java_files: Vec<PathBuf>,          // Track analyzed files for incremental updates
}

impl JavaProviderState {
    pub fn new() -> Self {
        Self {
            config: None,
            type_resolver: None,
            initialized: false,
            source_path: None,
            java_files: Vec::new(),
        }
    }
}

/// Java provider service implementation
#[derive(Clone)]
pub struct JavaProvider {
    state: Arc<RwLock<JavaProviderState>>,
}

impl JavaProvider {
    pub fn new() -> Self {
        info!("Creating JavaProvider instance");
        Self {
            state: Arc::new(RwLock::new(JavaProviderState::new())),
        }
    }

    /// Parse location type from string
    fn parse_location_type(location: &str) -> Result<LocationType> {
        match location.to_lowercase().as_str() {
            "import" => Ok(LocationType::Import),
            "package" => Ok(LocationType::Package),
            "class" | "type" => Ok(LocationType::Class),
            "field" | "field_declaration" | "fielddeclaration" => Ok(LocationType::Field),
            "method" => Ok(LocationType::Method),
            "enum" => Ok(LocationType::Enum),
            "inheritance" => Ok(LocationType::Inheritance),
            "implements" | "implements_type" | "implementstype" => Ok(LocationType::ImplementsType),
            "method_call" | "methodcall" => Ok(LocationType::MethodCall),
            "constructor_call" | "constructorcall" => Ok(LocationType::ConstructorCall),
            "annotation" => Ok(LocationType::Annotation),
            "variable" | "variable_declaration" | "variabledeclaration" => Ok(LocationType::Variable),
            "return_type" | "returntype" => Ok(LocationType::ReturnType),
            _ => anyhow::bail!("Unknown location type: {}", location),
        }
    }

    /// Convert our query results to incident contexts
    fn results_to_incidents(
        results: Vec<crate::java_graph::query::QueryResult>,
    ) -> Vec<IncidentContext> {
        results
            .into_iter()
            .map(|r| {
                IncidentContext {
                    file_uri: format!("file://{}", r.file_path),
                    effort: None,
                    code_location: Some(Location {
                        start_position: Some(Position {
                            line: r.line_number as f64,
                            character: r.column as f64,
                        }),
                        end_position: Some(Position {
                            line: r.line_number as f64,
                            character: (r.column + 10) as f64, // Approximate end position
                        }),
                    }),
                    line_number: Some(r.line_number as i64),
                    variables: None,
                    links: vec![],
                    is_dependency_incident: false,
                }
            })
            .collect()
    }
}

#[tonic::async_trait]
impl ProviderService for JavaProvider {
    async fn capabilities(
        &self,
        _request: Request<()>,
    ) -> std::result::Result<Response<CapabilitiesResponse>, Status> {
        info!("Capabilities requested");

        // Return the capabilities this provider supports
        let capabilities = vec![
            Capability {
                name: "referenced".to_string(),
                template_context: None,
            },
            Capability {
                name: "java".to_string(),
                template_context: None,
            },
            Capability {
                name: "dependency".to_string(),
                template_context: None,
            },
        ];

        Ok(Response::new(CapabilitiesResponse { capabilities }))
    }

    async fn init(
        &self,
        request: Request<Config>,
    ) -> std::result::Result<Response<InitResponse>, Status> {
        let config = request.into_inner();
        info!("Init requested with location: {}", config.location);

        let mut state = self.state.write().await;

        // Validate the source location exists
        let source_path = PathBuf::from(&config.location);
        if !source_path.exists() {
            return Ok(Response::new(InitResponse {
                error: format!("Source path does not exist: {}", config.location),
                successful: false,
                id: 0,
                builtin_config: None,
            }));
        }

        // Store configuration
        state.config = Some(config.clone());
        state.source_path = Some(source_path.clone());

        // Create TypeResolver and analyze files
        let mut type_resolver = TypeResolver::new();

        // Find all .java files
        let java_files = Self::find_java_files(&source_path)
            .map_err(|e| Status::internal(format!("Failed to find Java files: {}", e)))?;

        info!("Found {} Java files to analyze", java_files.len());

        // Analyze each file
        for java_file in &java_files {
            match type_resolver.analyze_file(java_file) {
                Ok(_) => debug!("Analyzed: {}", java_file.display()),
                Err(e) => warn!("Failed to analyze {}: {}", java_file.display(), e),
            }
        }

        // Build global index
        type_resolver.build_global_index();
        type_resolver.build_inheritance_maps();

        info!("Analysis complete. Indexed {} files", type_resolver.file_infos.len());

        // Store the resolver and file list (for incremental updates)
        state.type_resolver = Some(type_resolver);
        state.java_files = java_files;
        state.initialized = true;

        Ok(Response::new(InitResponse {
            error: String::new(),
            successful: true,
            id: 1,
            builtin_config: Some(config),
        }))
    }

    async fn evaluate(
        &self,
        request: Request<EvaluateRequest>,
    ) -> std::result::Result<Response<EvaluateResponse>, Status> {
        let req = request.into_inner();
        info!("Evaluate requested: cap={}, id={}", req.cap, req.id);

        // DEBUG: Write condition info to file for inspection
        if let Err(e) = std::fs::write("/tmp/condition_info.yaml", &req.condition_info) {
            warn!("Failed to write condition info to file: {}", e);
        }

        info!("RAW Condition info length: {} bytes", req.condition_info.len());

        let state = self.state.read().await;

        // Check if initialized
        if !state.initialized {
            return Ok(Response::new(EvaluateResponse {
                error: "Provider not initialized".to_string(),
                successful: false,
                response: None,
            }));
        }

        // Parse the condition YAML (analyzer sends YAML format)
        let condition_wrapper: ConditionWrapper = match serde_yaml::from_str(&req.condition_info) {
            Ok(c) => c,
            Err(e) => {
                return Ok(Response::new(EvaluateResponse {
                    error: format!("Failed to parse condition: {}", e),
                    successful: false,
                    response: None,
                }));
            }
        };

        let referenced = &condition_wrapper.referenced;

        // Parse location type
        let location_type = match Self::parse_location_type(&referenced.location) {
            Ok(lt) => lt,
            Err(e) => {
                return Ok(Response::new(EvaluateResponse {
                    error: format!("Invalid location type: {}", e),
                    successful: false,
                    response: None,
                }));
            }
        };

        // Parse pattern
        let pattern = match Pattern::from_string(&referenced.pattern) {
            Ok(p) => p,
            Err(e) => {
                return Ok(Response::new(EvaluateResponse {
                    error: format!("Invalid pattern: {}", e),
                    successful: false,
                    response: None,
                }));
            }
        };

        // Build query
        // TODO: Enhance query module to support full AnnotatedCondition with elements
        let annotated_pattern = referenced.annotated.as_ref().and_then(|a| a.pattern.clone());
        let query = ReferencedQuery {
            pattern,
            location: location_type,
            annotated: annotated_pattern,
            filters: None,  // Advanced filters not yet exposed via gRPC
        };

        // Get type resolver and file list
        let type_resolver = state.type_resolver.as_ref().unwrap().clone();
        let java_files = state.java_files.clone();
        drop(state); // Release read lock before potentially long operation

        // Build graph (optimization: we don't re-analyze files, just rebuild graph structure)
        let graph = loader::build_graph_for_files(&java_files.iter().map(|p| p.as_path()).collect::<Vec<_>>())
            .map_err(|e| Status::internal(format!("Failed to build graph: {}", e)))?;

        // Create query engine
        let engine = QueryEngine::new(graph, type_resolver);

        // Execute query
        let results = match engine.query(&query) {
            Ok(r) => r,
            Err(e) => {
                return Ok(Response::new(EvaluateResponse {
                    error: format!("Query execution failed: {}", e),
                    successful: false,
                    response: None,
                }));
            }
        };

        info!("Query returned {} results", results.len());

        // Convert results to incidents
        let incidents = Self::results_to_incidents(results);
        let matched = !incidents.is_empty();

        Ok(Response::new(EvaluateResponse {
            error: String::new(),
            successful: true,
            response: Some(ProviderEvaluateResponse {
                matched,
                incident_contexts: incidents,
                template_context: None,
            }),
        }))
    }

    async fn stop(
        &self,
        _request: Request<ServiceRequest>,
    ) -> std::result::Result<Response<()>, Status> {
        info!("Stop requested");
        Ok(Response::new(()))
    }

    async fn get_dependencies(
        &self,
        _request: Request<ServiceRequest>,
    ) -> std::result::Result<Response<DependencyResponse>, Status> {
        info!("GetDependencies requested");

        let state = self.state.read().await;

        // Check if initialized
        if !state.initialized {
            return Ok(Response::new(DependencyResponse {
                successful: false,
                error: "Provider not initialized".to_string(),
                file_dep: vec![],
            }));
        }

        let source_path = state.source_path.as_ref().unwrap().clone();
        drop(state); // Release lock

        // Detect build tool
        let build_tool = detect_build_tool(&source_path);

        match build_tool {
            BuildTool::Maven => {
                match self.resolve_maven_dependencies(&source_path).await {
                    Ok(file_deps) => Ok(Response::new(DependencyResponse {
                        successful: true,
                        error: String::new(),
                        file_dep: file_deps,
                    })),
                    Err(e) => Ok(Response::new(DependencyResponse {
                        successful: false,
                        error: format!("Failed to resolve Maven dependencies: {}", e),
                        file_dep: vec![],
                    })),
                }
            }
            BuildTool::Gradle => {
                match self.resolve_gradle_dependencies(&source_path).await {
                    Ok(file_deps) => Ok(Response::new(DependencyResponse {
                        successful: true,
                        error: String::new(),
                        file_dep: file_deps,
                    })),
                    Err(e) => Ok(Response::new(DependencyResponse {
                        successful: false,
                        error: format!("Failed to resolve Gradle dependencies: {}", e),
                        file_dep: vec![],
                    })),
                }
            }
            BuildTool::Unknown => {
                info!("No build tool detected (Maven or Gradle)");
                Ok(Response::new(DependencyResponse {
                    successful: true,
                    error: String::new(),
                    file_dep: vec![],
                }))
            }
        }
    }

    async fn get_dependencies_dag(
        &self,
        _request: Request<ServiceRequest>,
    ) -> std::result::Result<Response<DependencyDagResponse>, Status> {
        info!("GetDependenciesDAG requested");

        // TODO: Implement dependency DAG in Tasks 2.7 and 2.8
        Ok(Response::new(DependencyDagResponse {
            successful: true,
            error: String::new(),
            file_dag_dep: vec![],
        }))
    }

    async fn notify_file_changes(
        &self,
        request: Request<NotifyFileChangesRequest>,
    ) -> std::result::Result<Response<NotifyFileChangesResponse>, Status> {
        let req = request.into_inner();
        info!("NotifyFileChanges requested with {} changes", req.changes.len());

        let mut state = self.state.write().await;

        if !state.initialized {
            return Ok(Response::new(NotifyFileChangesResponse {
                error: "Provider not initialized".to_string(),
            }));
        }

        // Re-analyze changed files
        let mut changed_files = Vec::new();
        for change in &req.changes {
            // Convert URI to path
            let uri = &change.uri;
            if uri.starts_with("file://") {
                let path = PathBuf::from(&uri[7..]);
                if path.extension().map_or(false, |ext| ext == "java") {
                    changed_files.push(path);
                }
            }
        }

        if changed_files.is_empty() {
            return Ok(Response::new(NotifyFileChangesResponse {
                error: String::new(),
            }));
        }

        info!("Re-analyzing {} changed Java files", changed_files.len());

        if let Some(ref mut type_resolver) = state.type_resolver {
            // Re-analyze changed files
            for file in &changed_files {
                match type_resolver.analyze_file(file) {
                    Ok(_) => debug!("Re-analyzed: {}", file.display()),
                    Err(e) => warn!("Failed to re-analyze {}: {}", file.display(), e),
                }
            }

            // Rebuild indexes
            type_resolver.build_global_index();
            type_resolver.build_inheritance_maps();

            info!("Indexes updated for {} changed files", changed_files.len());
        }

        Ok(Response::new(NotifyFileChangesResponse {
            error: String::new(),
        }))
    }

    async fn prepare(
        &self,
        _request: Request<PrepareRequest>,
    ) -> std::result::Result<Response<PrepareResponse>, Status> {
        info!("Prepare requested");

        // Prepare is already done in Init for now
        Ok(Response::new(PrepareResponse {
            error: String::new(),
        }))
    }

    type StreamPrepareProgressStream = tokio_stream::wrappers::ReceiverStream<std::result::Result<ProgressEvent, Status>>;

    async fn stream_prepare_progress(
        &self,
        _request: Request<PrepareProgressRequest>,
    ) -> std::result::Result<Response<Self::StreamPrepareProgressStream>, Status> {
        info!("StreamPrepareProgress requested");

        let state = self.state.read().await;

        // Get file count
        let total_files = state.java_files.len() as i32;
        let files_processed = if state.initialized { total_files } else { 0 };

        drop(state);

        // Create a channel for streaming progress
        let (tx, rx) = tokio::sync::mpsc::channel(10);

        // Send current progress
        let _ = tx.send(Ok(ProgressEvent {
            r#type: 0, // PREPARE
            provider_name: "java".to_string(),
            files_processed,
            total_files,
        })).await;

        Ok(Response::new(tokio_stream::wrappers::ReceiverStream::new(rx)))
    }
}

impl JavaProvider {
    /// Recursively find all .java files in a directory
    fn find_java_files(path: &Path) -> Result<Vec<PathBuf>> {
        let mut java_files = Vec::new();

        if path.is_file() && path.extension().map_or(false, |ext| ext == "java") {
            java_files.push(path.to_path_buf());
        } else if path.is_dir() {
            for entry in std::fs::read_dir(path)
                .with_context(|| format!("Failed to read directory: {}", path.display()))?
            {
                let entry = entry?;
                let entry_path = entry.path();
                java_files.extend(Self::find_java_files(&entry_path)?);
            }
        }

        Ok(java_files)
    }

    /// Resolve Maven dependencies for a project
    async fn resolve_maven_dependencies(&self, source_path: &Path) -> Result<Vec<FileDep>> {
        info!("Resolving Maven dependencies for: {}", source_path.display());

        // Find all pom.xml files
        let pom_files = find_pom_files(source_path)?;

        info!("Found {} pom.xml file(s)", pom_files.len());
        for pom_file in &pom_files {
            info!("  - {}", pom_file.display());
        }

        if pom_files.is_empty() {
            info!("No pom.xml files found");
            return Ok(vec![]);
        }

        let mut file_deps = Vec::new();

        for pom_path in pom_files {
            let resolver = MavenResolver::new(pom_path.clone());

            // Resolve dependencies (will use mvn if available, otherwise parse pom.xml)
            match resolver.resolve_dependencies() {
                Ok(maven_deps) => {
                    info!("Resolved {} Maven dependencies from {}", maven_deps.len(), pom_path.display());
                    for md in &maven_deps {
                        debug!("  - {}:{}", md.group_id, md.artifact_id);
                    }

                    // Convert Maven dependencies to protobuf Dependency format
                    let proto_deps: Vec<Dependency> = maven_deps
                        .iter()
                        .map(|md| Dependency {
                            name: md.name().to_string(),
                            version: md.version.clone().unwrap_or_default(),
                            classifier: md.classifier.clone().unwrap_or_default(),
                            r#type: md.type_.clone().unwrap_or_else(|| "jar".to_string()),
                            resolved_identifier: md.to_identifier(),
                            file_uri_prefix: format!("file://{}", pom_path.display()),
                            indirect: false, // We'll mark direct dependencies as false
                            extras: None,
                            labels: vec![],
                        })
                        .collect();

                    file_deps.push(FileDep {
                        file_uri: format!("file://{}", pom_path.display()),
                        list: Some(DependencyList { deps: proto_deps }),
                    });

                    info!("Resolved {} dependencies for {}", maven_deps.len(), pom_path.display());
                }
                Err(e) => {
                    warn!("Failed to resolve dependencies for {}: {}", pom_path.display(), e);
                }
            }
        }

        Ok(file_deps)
    }

    /// Resolve Gradle dependencies for a project
    async fn resolve_gradle_dependencies(&self, source_path: &Path) -> Result<Vec<FileDep>> {
        info!("Resolving Gradle dependencies for: {}", source_path.display());

        // Find all build.gradle and build.gradle.kts files
        let gradle_files = find_gradle_files(source_path)?;

        info!("Found {} Gradle build file(s)", gradle_files.len());
        for gradle_file in &gradle_files {
            info!("  - {}", gradle_file.display());
        }

        if gradle_files.is_empty() {
            info!("No Gradle build files found");
            return Ok(vec![]);
        }

        let mut file_deps = Vec::new();

        for gradle_file in gradle_files {
            // Check for Gradle wrapper
            let wrapper_path = gradle_file.parent()
                .and_then(|p| p.parent())
                .map(|p| p.join("gradlew"));

            let resolver = if wrapper_path.as_ref().map_or(false, |p| p.exists()) {
                GradleResolver::new(gradle_file.clone())
                    .with_gradle_cmd("./gradlew".to_string())
            } else {
                GradleResolver::new(gradle_file.clone())
            };

            // Resolve dependencies (will use gradle if available, otherwise parse build file)
            match resolver.resolve_dependencies() {
                Ok(gradle_deps) => {
                    info!("Resolved {} Gradle dependencies from {}", gradle_deps.len(), gradle_file.display());
                    for gd in &gradle_deps {
                        debug!("  - {}:{}", gd.group, gd.name);
                    }

                    // Convert Gradle dependencies to protobuf Dependency format
                    let proto_deps: Vec<Dependency> = gradle_deps
                        .iter()
                        .map(|gd| Dependency {
                            name: gd.artifact_name().to_string(),
                            version: gd.version.clone().unwrap_or_default(),
                            classifier: String::new(),
                            r#type: "jar".to_string(),
                            resolved_identifier: gd.to_identifier(),
                            file_uri_prefix: format!("file://{}", gradle_file.display()),
                            indirect: false,
                            extras: None,
                            labels: gd.configuration.as_ref().map(|c| vec![c.clone()]).unwrap_or_default(),
                        })
                        .collect();

                    file_deps.push(FileDep {
                        file_uri: format!("file://{}", gradle_file.display()),
                        list: Some(DependencyList { deps: proto_deps }),
                    });

                    info!("Resolved {} dependencies for {}", gradle_deps.len(), gradle_file.display());
                }
                Err(e) => {
                    warn!("Failed to resolve dependencies for {}: {}", gradle_file.display(), e);
                }
            }
        }

        Ok(file_deps)
    }
}

/// Implementation of code location service for providing code snippets
#[tonic::async_trait]
impl ProviderCodeLocationService for JavaProvider {
    async fn get_code_snip(
        &self,
        request: Request<GetCodeSnipRequest>,
    ) -> std::result::Result<Response<GetCodeSnipResponse>, Status> {
        let req = request.into_inner();

        // Extract file path from URI (remove "file://" prefix)
        let file_path = req.uri.trim_start_matches("file://");

        // Get location info
        let location = req.code_location
            .ok_or_else(|| Status::invalid_argument("Missing code location"))?;

        let line_number = location.start_position
            .as_ref()
            .map(|p| p.line as usize)
            .unwrap_or(0);

        // Read the file and extract snippet with context
        let snippet = match std::fs::read_to_string(file_path) {
            Ok(content) => {
                let lines: Vec<&str> = content.lines().collect();

                // Get 3 lines of context before and after (or fewer if near file boundaries)
                let context = 3;
                let start = line_number.saturating_sub(context).max(1);
                let end = (line_number + context + 1).min(lines.len());

                // Build snippet with line numbers
                let mut snippet_lines = Vec::new();
                for (idx, line) in lines.iter().enumerate().skip(start.saturating_sub(1)).take(end - start + 1) {
                    let line_num = idx + 1;
                    let marker = if line_num == line_number { ">>> " } else { "    " };
                    snippet_lines.push(format!("{}{:4} | {}", marker, line_num, line));
                }

                snippet_lines.join("\n")
            }
            Err(e) => {
                warn!("Failed to read file for code snippet: {} - {}", file_path, e);
                format!("// Unable to read file: {}", e)
            }
        };

        Ok(Response::new(GetCodeSnipResponse { snip: snippet }))
    }
}

impl Default for JavaProvider {
    fn default() -> Self {
        Self::new()
    }
}
