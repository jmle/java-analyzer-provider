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
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referenced: Option<ReferencedCondition>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dependency: Option<DependencyCondition>,
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

/// Dependency matching condition (for java.dependency capability)
#[derive(Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DependencyCondition {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lowerbound: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub upperbound: Option<String>,
}

/// Semantic version comparison
/// Returns -1 if v1 < v2, 0 if v1 == v2, 1 if v1 > v2
fn compare_versions(v1: &str, v2: &str) -> i32 {
    let parse_version = |v: &str| -> Vec<u32> {
        v.split('.')
            .filter_map(|part| part.parse::<u32>().ok())
            .collect()
    };

    let parts1 = parse_version(v1);
    let parts2 = parse_version(v2);

    let max_len = parts1.len().max(parts2.len());

    for i in 0..max_len {
        let p1 = parts1.get(i).copied().unwrap_or(0);
        let p2 = parts2.get(i).copied().unwrap_or(0);

        if p1 < p2 {
            return -1;
        } else if p1 > p2 {
            return 1;
        }
    }

    0
}

/// Check if a version is within the specified bounds (inclusive)
fn version_in_range(version: &str, lowerbound: Option<&str>, upperbound: Option<&str>) -> bool {
    if let Some(lower) = lowerbound {
        if compare_versions(version, lower) < 0 {
            return false;
        }
    }

    if let Some(upper) = upperbound {
        if compare_versions(version, upper) > 0 {
            return false;
        }
    }

    true
}

/// State of the Java provider
/// State for a single provider instance (one Init call)
#[derive(Clone)]
pub struct ProviderInstanceState {
    config: Config,
    source_path: PathBuf,
    initialized: bool,
}

/// Global provider state managing multiple instances
pub struct JavaProviderState {
    instances: HashMap<i64, ProviderInstanceState>,
    next_id: i64,
    // Global type resolver shared across all instances
    type_resolver: Option<TypeResolver>,
    java_files: Vec<PathBuf>,
}

impl JavaProviderState {
    pub fn new() -> Self {
        Self {
            instances: HashMap::new(),
            next_id: 1,
            type_resolver: None,
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

    /// Create a new provider instance that shares state with another
    pub fn new_with_shared_state(state: Arc<RwLock<JavaProviderState>>) -> Self {
        info!("Creating JavaProvider instance with shared state");
        Self { state }
    }

    /// Get a reference to the shared state (for creating additional instances)
    pub fn get_shared_state(&self) -> Arc<RwLock<JavaProviderState>> {
        Arc::clone(&self.state)
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

    /// Evaluate a dependency condition
    async fn evaluate_dependency_condition(
        &self,
        dependency_cond: &DependencyCondition,
        source_path: &Path,
    ) -> std::result::Result<Response<EvaluateResponse>, Status> {
        info!("Evaluating dependency condition: name={} for path={}", dependency_cond.name, source_path.display());

        // Convert dot notation to colon notation (junit.junit -> junit:junit)
        let target_name = dependency_cond.name.replace('.', ":");

        let mut incidents = Vec::new();

        // Check Maven dependencies
        let pom_files = find_pom_files(&source_path)
            .map_err(|e| Status::internal(format!("Failed to find pom files: {}", e)))?;

        for pom_path in pom_files {
            if let Ok(deps) = self.get_maven_dependencies_with_lines(&pom_path).await {
                for (dep, line_num) in deps {
                    let dep_name = format!("{}:{}", dep.group_id, dep.artifact_id);

                    if dep_name == target_name {
                        if let Some(ref version) = dep.version {
                            // Check version bounds
                            if version_in_range(
                                version,
                                dependency_cond.lowerbound.as_deref(),
                                dependency_cond.upperbound.as_deref(),
                            ) {
                                incidents.push(self.create_dependency_incident(
                                    &pom_path,
                                    line_num,
                                    &dependency_cond.name,
                                    version,
                                ));
                            }
                        }
                    }
                }
            }
        }

        // Check Gradle dependencies
        let gradle_files = find_gradle_files(&source_path)
            .map_err(|e| Status::internal(format!("Failed to find gradle files: {}", e)))?;

        for gradle_file in gradle_files {
            if let Ok(deps) = self.get_gradle_dependencies_with_lines(&gradle_file).await {
                for (dep, line_num) in deps {
                    let dep_name = format!("{}:{}", dep.group_id, dep.artifact_id);

                    if dep_name == target_name {
                        if let Some(ref version) = dep.version {
                            if version_in_range(
                                version,
                                dependency_cond.lowerbound.as_deref(),
                                dependency_cond.upperbound.as_deref(),
                            ) {
                                incidents.push(self.create_dependency_incident(
                                    &gradle_file,
                                    line_num,
                                    &dependency_cond.name,
                                    version,
                                ));
                            }
                        }
                    }
                }
            }
        }

        let matched = !incidents.is_empty();
        info!("Dependency condition matched: {} incidents found", incidents.len());

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

    /// Get Maven dependencies with their line numbers from a pom.xml file
    async fn get_maven_dependencies_with_lines(&self, pom_path: &Path) -> Result<Vec<(crate::buildtool::maven::MavenDependency, u32)>> {
        use crate::buildtool::maven::MavenPom;
        use quick_xml::events::Event;
        use quick_xml::Reader;
        use std::fs::read_to_string;

        let pom = MavenPom::parse(pom_path)?;
        let xml_content = read_to_string(pom_path)?;

        let mut deps_with_lines = Vec::new();
        let mut reader = Reader::from_str(&xml_content);
        reader.config_mut().trim_text(true);

        let mut in_dependency = false;
        let mut current_element = String::new();
        let mut current_group_id = String::new();
        let mut current_artifact_id = String::new();
        let mut current_version = None;
        let mut dependency_start_line = 0u32;

        // Track line numbers by counting newlines in the raw text
        let mut current_line = 1u32;

        let mut buf = Vec::new();
        loop {
            let event_pos = reader.buffer_position() as usize;
            // Count newlines up to current position
            let content_bytes = xml_content.as_bytes();
            if event_pos < content_bytes.len() {
                current_line = content_bytes[..event_pos].iter().filter(|&&b| b == b'\n').count() as u32 + 1;
            }

            match reader.read_event_into(&mut buf) {
                Ok(Event::Start(e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name == "dependency" {
                        in_dependency = true;
                        dependency_start_line = current_line;
                        current_group_id.clear();
                        current_artifact_id.clear();
                        current_version = None;
                    } else if in_dependency {
                        current_element = name;
                    }
                }
                Ok(Event::Text(e)) => {
                    if in_dependency && !current_element.is_empty() {
                        let text = e.unescape().unwrap_or_default().to_string();
                        let resolved_text = pom.resolve_version(&text);

                        match current_element.as_str() {
                            "groupId" => current_group_id = resolved_text,
                            "artifactId" => current_artifact_id = resolved_text,
                            "version" => current_version = Some(resolved_text),
                            _ => {}
                        }
                    }
                }
                Ok(Event::End(e)) => {
                    let name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                    if name == "dependency" && in_dependency {
                        // Found a complete dependency
                        if !current_group_id.is_empty() && !current_artifact_id.is_empty() {
                            deps_with_lines.push((
                                crate::buildtool::maven::MavenDependency {
                                    group_id: current_group_id.clone(),
                                    artifact_id: current_artifact_id.clone(),
                                    version: current_version.clone(),
                                    scope: None,
                                    classifier: None,
                                    type_: None,
                                    optional: false,
                                },
                                dependency_start_line,
                            ));
                        }
                        in_dependency = false;
                    } else if in_dependency && name == current_element {
                        current_element.clear();
                    }
                }
                Ok(Event::Eof) => break,
                Err(e) => {
                    warn!("Error parsing pom.xml at position {}: {:?}", reader.buffer_position(), e);
                    break;
                }
                _ => {}
            }
            buf.clear();
        }

        Ok(deps_with_lines)
    }

    /// Get Gradle dependencies with their line numbers from a build.gradle file
    async fn get_gradle_dependencies_with_lines(&self, gradle_path: &Path) -> Result<Vec<(crate::buildtool::maven::MavenDependency, u32)>> {
        use std::fs::read_to_string;
        use regex::Regex;

        let content = read_to_string(gradle_path)?;
        let mut deps_with_lines = Vec::new();

        // Match patterns like: compile 'junit:junit:4.12' or implementation "io.fabric8:kubernetes-client:6.0.0"
        let dep_regex = Regex::new(r#"(?m)^\s*(?:compile|implementation|testImplementation|api|testCompile)\s+['"]([^:'"]+):([^:'"]+):([^'"]+)['"]"#)?;

        for (line_num, line) in content.lines().enumerate() {
            if let Some(caps) = dep_regex.captures(line) {
                let group_id = caps.get(1).map(|m| m.as_str().to_string()).unwrap_or_default();
                let artifact_id = caps.get(2).map(|m| m.as_str().to_string()).unwrap_or_default();
                let version = caps.get(3).map(|m| m.as_str().to_string());

                if !group_id.is_empty() && !artifact_id.is_empty() {
                    deps_with_lines.push((
                        crate::buildtool::maven::MavenDependency {
                            group_id,
                            artifact_id,
                            version,
                            scope: None,
                            classifier: None,
                            type_: None,
                            optional: false,
                        },
                        (line_num + 1) as u32,
                    ));
                }
            }
        }

        Ok(deps_with_lines)
    }

    /// Extract code snippet around a line number
    fn extract_code_snippet(file_path: &Path, line_num: u32, before: usize, after: usize) -> String {
        use std::fs::read_to_string;

        let content = match read_to_string(file_path) {
            Ok(c) => c,
            Err(_) => return String::new(),
        };

        let lines: Vec<&str> = content.lines().collect();
        let target_line = (line_num as usize).saturating_sub(1);

        let start = target_line.saturating_sub(before);
        let end = (target_line + after + 1).min(lines.len());

        let mut snippet = String::new();
        for (i, line) in lines[start..end].iter().enumerate() {
            let line_number = start + i + 1;
            if line_number == line_num as usize {
                snippet.push_str(&format!(">>> {:3} | {}\n", line_number, line));
            } else {
                snippet.push_str(&format!("    {:3} | {}\n", line_number, line));
            }
        }

        snippet
    }

    /// Create an incident for a matched dependency
    fn create_dependency_incident(&self, file_path: &Path, line_num: u32, dep_name: &str, version: &str) -> IncidentContext {
        use prost_types::{Struct, Value};

        // Create variables for template substitution
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("name".to_string(), Value {
            kind: Some(prost_types::value::Kind::StringValue(dep_name.to_string())),
        });
        fields.insert("version".to_string(), Value {
            kind: Some(prost_types::value::Kind::StringValue(version.to_string())),
        });

        IncidentContext {
            file_uri: format!("file://{}", file_path.display()),
            effort: None,
            code_location: None,
            line_number: Some(line_num as i64),
            variables: Some(Struct { fields }),
            links: vec![],
            is_dependency_incident: true,
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

        // Create new instance with unique ID
        let instance_id = state.next_id;
        state.next_id += 1;

        // Get or create TypeResolver (accumulate across multiple Init calls)
        let mut type_resolver = state.type_resolver.take().unwrap_or_else(|| TypeResolver::new());

        // Find all .java files in this location
        let java_files = Self::find_java_files(&source_path)
            .map_err(|e| Status::internal(format!("Failed to find Java files: {}", e)))?;

        info!("Found {} Java files to analyze in {}", java_files.len(), config.location);

        // Analyze each file and add to resolver
        for java_file in &java_files {
            match type_resolver.analyze_file(java_file) {
                Ok(_) => debug!("Analyzed: {}", java_file.display()),
                Err(e) => warn!("Failed to analyze {}: {}", java_file.display(), e),
            }
        }

        // Append to accumulated file list
        state.java_files.extend(java_files);

        // Rebuild global indices with ALL accumulated files
        type_resolver.build_global_index();
        type_resolver.build_inheritance_maps();
        type_resolver.resolve_annotation_fqdns();

        info!("Analysis complete. Total indexed files: {}", type_resolver.file_infos.len());

        // Store the accumulated resolver back
        state.type_resolver = Some(type_resolver);

        // Create and store instance state
        let instance_state = ProviderInstanceState {
            config: config.clone(),
            source_path,
            initialized: true,
        };
        state.instances.insert(instance_id, instance_state);

        info!("Created provider instance with ID: {}", instance_id);

        Ok(Response::new(InitResponse {
            error: String::new(),
            successful: true,
            id: instance_id,
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

        // Look up instance by ID
        let instance = match state.instances.get(&req.id) {
            Some(inst) => inst.clone(),
            None => {
                return Ok(Response::new(EvaluateResponse {
                    error: format!("Unknown instance ID: {}", req.id),
                    successful: false,
                    response: None,
                }));
            }
        };

        // Check if initialized
        if !instance.initialized {
            return Ok(Response::new(EvaluateResponse {
                error: "Provider not initialized".to_string(),
                successful: false,
                response: None,
            }));
        }

        // Parse the condition YAML (analyzer sends YAML format)
        let condition_wrapper: ConditionWrapper = match serde_yaml::from_str::<ConditionWrapper>(&req.condition_info) {
            Ok(c) => {
                info!("Parsed condition - ruleID: {}, has_referenced: {}, has_dependency: {}",
                      c.rule_id, c.referenced.is_some(), c.dependency.is_some());
                c
            },
            Err(e) => {
                warn!("Failed to parse condition: {}", e);
                return Ok(Response::new(EvaluateResponse {
                    error: format!("Failed to parse condition: {}", e),
                    successful: false,
                    response: None,
                }));
            }
        };

        // Check which capability is being evaluated
        if let Some(ref dependency_cond) = condition_wrapper.dependency {
            // Handle java.dependency capability
            info!("Handling dependency condition for rule: {}", condition_wrapper.rule_id);
            let source_path = instance.source_path.clone();
            drop(state); // Release lock before potentially long operation
            return self.evaluate_dependency_condition(dependency_cond, &source_path).await;
        }

        // Handle java.referenced capability
        let referenced = match &condition_wrapper.referenced {
            Some(r) => r,
            None => {
                return Ok(Response::new(EvaluateResponse {
                    error: "No referenced or dependency condition found".to_string(),
                    successful: false,
                    response: None,
                }));
            }
        };

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

        // Build query with annotation filter if present
        let annotation_filter = referenced.annotated.as_ref().map(|annotated_cond| {
            use crate::java_graph::query::AnnotationFilter;
            use std::collections::HashMap;

            let mut elements = HashMap::new();
            for element in &annotated_cond.elements {
                elements.insert(element.name.clone(), element.value.clone());
            }

            tracing::debug!(
                "Building annotation filter - pattern: {:?}, elements: {:?}",
                annotated_cond.pattern,
                elements
            );

            AnnotationFilter {
                pattern: annotated_cond.pattern.clone(),
                elements,
            }
        });

        let query = ReferencedQuery {
            pattern,
            location: location_type,
            annotated: annotation_filter,
            filters: None,  // Advanced filters not yet exposed via gRPC
        };

        // Get type resolver
        let type_resolver = state.type_resolver.as_ref().unwrap().clone();
        drop(state); // Release read lock before potentially long operation

        // Create query engine (no graph needed - all queries use TypeResolver)
        let engine = QueryEngine::new(type_resolver);

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
        request: Request<ServiceRequest>,
    ) -> std::result::Result<Response<DependencyResponse>, Status> {
        let req = request.into_inner();
        info!("GetDependencies requested for instance ID: {}", req.id);

        let state = self.state.read().await;

        // Look up instance by ID
        let instance = match state.instances.get(&req.id) {
            Some(inst) => inst.clone(),
            None => {
                return Ok(Response::new(DependencyResponse {
                    successful: false,
                    error: format!("Unknown instance ID: {}", req.id),
                    file_dep: vec![],
                }));
            }
        };

        // Check if initialized
        if !instance.initialized {
            return Ok(Response::new(DependencyResponse {
                successful: false,
                error: "Provider not initialized".to_string(),
                file_dep: vec![],
            }));
        }

        let source_path = instance.source_path.clone();
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
                info!("No build tool detected (Maven or Gradle) for path: {}", source_path.display());
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

        // If no instances exist, nothing to notify
        if state.instances.is_empty() {
            return Ok(Response::new(NotifyFileChangesResponse {
                error: "No provider instances initialized".to_string(),
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

            // Rebuild indexes and resolve annotations
            type_resolver.build_global_index();
            type_resolver.build_inheritance_maps();
            type_resolver.resolve_annotation_fqdns();

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
        let files_processed = total_files; // All files in the list have been processed

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
                            // Use groupId.artifactId format for name (matches analyzer-lsp expectation)
                            name: format!("{}.{}", gd.group, gd.name),
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
