// TypeResolver - Custom semantic layer for Java
// Handles import resolution, type name resolution, and symbol table management

use anyhow::{Context, Result};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use tree_sitter::Tree;

use super::ast_explorer;
use super::language_config;

/// Source code position (line and column)
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourcePosition {
    pub line: usize,        // 1-based line number
    pub column: usize,      // 0-based column number
    pub end_line: usize,    // 1-based line number
    pub end_column: usize,  // 0-based column number
}

impl SourcePosition {
    /// Create from tree-sitter node
    pub fn from_node(node: tree_sitter::Node) -> Self {
        let start = node.start_position();
        let end = node.end_position();

        SourcePosition {
            line: start.row + 1,        // tree-sitter uses 0-based, we use 1-based
            column: start.column,
            end_line: end.row + 1,
            end_column: end.column,
        }
    }

    /// Create a default position (unknown location)
    pub fn unknown() -> Self {
        SourcePosition {
            line: 0,
            column: 0,
            end_line: 0,
            end_column: 0,
        }
    }
}

/// Information about a single Java source file's symbols
#[derive(Debug, Clone)]
pub struct FileInfo {
    pub file_path: PathBuf,
    pub package_name: Option<String>,
    pub explicit_imports: HashMap<String, String>,  // "List" -> "java.util.List"
    pub wildcard_imports: Vec<String>,              // ["java.util", "java.io"]
    pub classes: HashMap<String, ClassInfo>,        // "Simple" -> ClassInfo
    pub method_calls: Vec<MethodCall>,              // Method invocations in this file
    pub constructor_calls: Vec<ConstructorCall>,    // Constructor invocations in this file
    pub annotations: Vec<AnnotationUsage>,          // Annotation usages in this file
    pub variables: Vec<VariableDeclaration>,        // Local variable declarations in this file
}

/// Information about a class, interface, or enum
#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub simple_name: String,
    pub fqdn: String,
    pub extends: Option<String>,      // Parent class (simple name, to be resolved later)
    pub implements: Vec<String>,      // Interface names (simple names, to be resolved later)
    pub methods: Vec<MethodInfo>,
    pub fields: Vec<FieldInfo>,
    pub annotations: Vec<AnnotationInfo>,  // Annotations on this class
    pub is_interface: bool,
    pub is_enum: bool,
    pub position: SourcePosition,
}

/// Information about a method
#[derive(Debug, Clone)]
pub struct MethodInfo {
    pub name: String,
    pub return_type: String,          // Simple name or primitive
    pub parameters: Vec<(String, String)>,  // (param_name, type_name)
    pub annotations: Vec<AnnotationInfo>,  // Annotations on this method
    pub position: SourcePosition,
}

/// Information about a field
#[derive(Debug, Clone)]
pub struct FieldInfo {
    pub name: String,
    pub type_name: String,            // Simple name or primitive
    pub annotations: Vec<AnnotationInfo>,  // Annotations on this field
    pub position: SourcePosition,
}

/// Information about a method call (invocation)
#[derive(Debug, Clone)]
pub struct MethodCall {
    pub method_name: String,
    pub receiver_type: Option<String>,  // Type of object being called (if known)
    pub position: SourcePosition,
}

/// Information about a constructor call (new expression)
#[derive(Debug, Clone)]
pub struct ConstructorCall {
    pub type_name: String,              // Type being instantiated (simple name)
    pub resolved_type: Option<String>,  // Resolved FQDN (if known)
    pub position: SourcePosition,
}

/// Information about an annotation usage
#[derive(Debug, Clone)]
pub struct AnnotationUsage {
    pub annotation_name: String,        // Annotation type (simple name, e.g., "Override")
    pub target: AnnotationTarget,       // What is being annotated
    pub position: SourcePosition,
}

/// What an annotation is attached to
#[derive(Debug, Clone)]
pub enum AnnotationTarget {
    Class(String),                      // Annotated class name
    Method(String, String),             // (class name, method name)
    Field(String, String),              // (class name, field name)
    Parameter(String, String, String),  // (class name, method name, param name)
    Unknown,                            // Couldn't determine target
}

/// Detailed annotation information with elements
#[derive(Debug, Clone)]
pub struct AnnotationInfo {
    pub name: String,                   // Annotation name (simple, e.g., "Inject")
    pub fqdn: Option<String>,           // Resolved FQDN (e.g., "javax.inject.Inject")
    pub elements: HashMap<String, String>,  // Annotation elements (e.g., {"name" => "id", "value" => "user_id"})
    pub position: SourcePosition,
}

/// Information about a local variable declaration
#[derive(Debug, Clone)]
pub struct VariableDeclaration {
    pub variable_name: String,          // Variable name (e.g., "count", "items")
    pub type_name: String,              // Type (simple name, e.g., "int", "List")
    pub resolved_type: Option<String>,  // Resolved FQDN (if known)
    pub method_context: Option<String>, // Method containing this variable
    pub class_context: Option<String>,  // Class containing this variable
    pub position: SourcePosition,
}

/// Global type resolver with cross-file analysis
#[derive(Clone)]
pub struct TypeResolver {
    pub file_infos: HashMap<PathBuf, FileInfo>,
    pub global_type_index: HashMap<String, Vec<String>>,  // "List" -> ["java.util.List", ...]
    pub inheritance_map: HashMap<String, String>,         // Child FQDN -> Parent FQDN
    pub interface_map: HashMap<String, Vec<String>>,      // Class FQDN -> Interface FQDNs
}

impl TypeResolver {
    /// Create a new TypeResolver
    pub fn new() -> Self {
        TypeResolver {
            file_infos: HashMap::new(),
            global_type_index: HashMap::new(),
            inheritance_map: HashMap::new(),
            interface_map: HashMap::new(),
        }
    }

    /// Analyze a Java file and extract symbol information
    pub fn analyze_file(&mut self, file_path: &Path) -> Result<()> {
        let file_info = Self::extract_file_info(file_path)?;
        self.file_infos.insert(file_path.to_path_buf(), file_info);
        Ok(())
    }

    /// Extract FileInfo from a Java source file
    fn extract_file_info(file_path: &Path) -> Result<FileInfo> {
        let (source, tree) = language_config::parse_file(file_path)
            .with_context(|| format!("Failed to parse {}", file_path.display()))?;

        let package_name = extract_package(&tree, &source);
        let (explicit_imports, wildcard_imports) = extract_imports(&tree, &source);
        let classes = extract_classes(&tree, &source, &package_name)?;
        let method_calls = extract_method_calls(&tree, &source);
        let constructor_calls = extract_constructor_calls(&tree, &source);
        let annotations = extract_annotations(&tree, &source, &classes);
        let variables = extract_variables(&tree, &source, &classes);

        Ok(FileInfo {
            file_path: file_path.to_path_buf(),
            package_name,
            explicit_imports,
            wildcard_imports,
            classes,
            method_calls,
            constructor_calls,
            annotations,
            variables,
        })
    }

    /// Build global type index from all analyzed files
    pub fn build_global_index(&mut self) {
        self.global_type_index.clear();

        for file_info in self.file_infos.values() {
            for class_info in file_info.classes.values() {
                let simple_name = &class_info.simple_name;
                let fqdn = &class_info.fqdn;

                self.global_type_index
                    .entry(simple_name.clone())
                    .or_default()
                    .push(fqdn.clone());
            }
        }
    }

    /// Build inheritance and interface maps from all analyzed files
    /// Should be called after build_global_index()
    pub fn build_inheritance_maps(&mut self) {
        self.inheritance_map.clear();
        self.interface_map.clear();

        // Collect all file paths first to avoid borrow issues
        let file_paths: Vec<PathBuf> = self.file_infos.keys().cloned().collect();

        for file_path in file_paths {
            if let Some(file_info) = self.file_infos.get(&file_path) {
                for class_info in file_info.classes.values() {
                    let class_fqdn = &class_info.fqdn;

                    // Resolve parent class (extends)
                    if let Some(parent_simple_name) = &class_info.extends {
                        if let Some(parent_fqdn) = self.resolve_type_name(parent_simple_name, &file_path) {
                            self.inheritance_map.insert(class_fqdn.clone(), parent_fqdn);
                        }
                    }

                    // Resolve interfaces (implements)
                    let mut interface_fqdns = Vec::new();
                    for interface_simple_name in &class_info.implements {
                        if let Some(interface_fqdn) = self.resolve_type_name(interface_simple_name, &file_path) {
                            interface_fqdns.push(interface_fqdn);
                        }
                    }
                    if !interface_fqdns.is_empty() {
                        self.interface_map.insert(class_fqdn.clone(), interface_fqdns);
                    }
                }
            }
        }
    }

    /// Resolve annotation FQDNs for all classes, methods, and fields
    /// Should be called after build_global_index()
    pub fn resolve_annotation_fqdns(&mut self) {
        let file_paths: Vec<PathBuf> = self.file_infos.keys().cloned().collect();

        for file_path in file_paths {
            // We need to clone and modify to avoid borrow checker issues
            if let Some(file_info) = self.file_infos.get(&file_path).cloned() {
                let mut updated_classes = file_info.classes.clone();

                for (class_name, class_info) in &mut updated_classes {
                    // Resolve class annotations
                    for annotation in &mut class_info.annotations {
                        if annotation.fqdn.is_none() {
                            annotation.fqdn = self.resolve_type_name(&annotation.name, &file_path);
                        }
                    }

                    // Resolve method annotations
                    for method in &mut class_info.methods {
                        for annotation in &mut method.annotations {
                            if annotation.fqdn.is_none() {
                                annotation.fqdn = self.resolve_type_name(&annotation.name, &file_path);
                            }
                        }
                    }

                    // Resolve field annotations
                    for field in &mut class_info.fields {
                        for annotation in &mut field.annotations {
                            if annotation.fqdn.is_none() {
                                annotation.fqdn = self.resolve_type_name(&annotation.name, &file_path);
                            }
                        }
                    }
                }

                // Update the file_info with resolved annotations
                if let Some(file_info_mut) = self.file_infos.get_mut(&file_path) {
                    file_info_mut.classes = updated_classes;
                }
            }
        }
    }

    /// Resolve a simple type name to FQDN
    pub fn resolve_type_name(&self, simple_name: &str, file_path: &Path) -> Option<String> {
        let file_info = self.file_infos.get(file_path)?;

        // Strategy 1: Primitives - return as-is
        if is_primitive(simple_name) {
            return Some(simple_name.to_string());
        }

        // Strategy 2: Explicit imports
        if let Some(fqdn) = file_info.explicit_imports.get(simple_name) {
            return Some(fqdn.clone());
        }

        // Strategy 3: Same package
        if let Some(pkg) = &file_info.package_name {
            if file_info.classes.contains_key(simple_name) {
                return Some(format!("{}.{}", pkg, simple_name));
            }

            // Check if it exists in same package via global index
            let candidate = format!("{}.{}", pkg, simple_name);
            if self.global_type_index
                .get(simple_name)
                .map(|fqdns| fqdns.contains(&candidate))
                .unwrap_or(false)
            {
                return Some(candidate);
            }
        } else if file_info.classes.contains_key(simple_name) {
            // Default package - just the simple name
            return Some(simple_name.to_string());
        }

        // Strategy 4: java.lang (implicit)
        if is_java_lang_type(simple_name) {
            return Some(format!("java.lang.{}", simple_name));
        }

        // Strategy 5: Wildcard imports
        for wildcard_pkg in &file_info.wildcard_imports {
            let candidate = format!("{}.{}", wildcard_pkg, simple_name);
            if self.global_type_index
                .get(simple_name)
                .map(|fqdns| fqdns.contains(&candidate))
                .unwrap_or(false)
            {
                return Some(candidate);
            }
        }

        // Strategy 6: Fallback - return None (unresolvable)
        None
    }

    /// Get the direct parent class FQDN for a given class
    pub fn get_parent_class(&self, class_fqdn: &str) -> Option<&String> {
        self.inheritance_map.get(class_fqdn)
    }

    /// Get all parent classes (transitive) for a given class
    /// Returns a vector of FQDNs from immediate parent to root
    pub fn get_all_parents(&self, class_fqdn: &str) -> Vec<String> {
        let mut parents = Vec::new();
        let mut current = class_fqdn;

        // Walk up the inheritance chain
        while let Some(parent) = self.inheritance_map.get(current) {
            parents.push(parent.clone());
            current = parent;

            // Prevent infinite loops (shouldn't happen in valid Java, but be safe)
            if parents.len() > 100 {
                break;
            }
        }

        parents
    }

    /// Check if a class extends a specific parent class (direct or transitive)
    /// Pattern can be simple name or FQDN
    pub fn extends_class(&self, class_fqdn: &str, parent_pattern: &str) -> bool {
        // Check direct parent
        if let Some(parent) = self.inheritance_map.get(class_fqdn) {
            if parent == parent_pattern || parent.ends_with(&format!(".{}", parent_pattern)) {
                return true;
            }
        }

        // Check transitive parents
        for parent in self.get_all_parents(class_fqdn) {
            if parent == parent_pattern || parent.ends_with(&format!(".{}", parent_pattern)) {
                return true;
            }
        }

        false
    }

    /// Get the direct interfaces implemented by a class
    pub fn get_interfaces(&self, class_fqdn: &str) -> Vec<String> {
        self.interface_map
            .get(class_fqdn)
            .cloned()
            .unwrap_or_default()
    }

    /// Get all interfaces (direct and transitive through parent classes)
    pub fn get_all_interfaces(&self, class_fqdn: &str) -> Vec<String> {
        let mut all_interfaces = Vec::new();
        let mut seen = std::collections::HashSet::new();

        // Get direct interfaces
        if let Some(interfaces) = self.interface_map.get(class_fqdn) {
            for interface in interfaces {
                if seen.insert(interface.clone()) {
                    all_interfaces.push(interface.clone());
                }
            }
        }

        // Get interfaces from parent classes
        for parent in self.get_all_parents(class_fqdn) {
            if let Some(interfaces) = self.interface_map.get(&parent) {
                for interface in interfaces {
                    if seen.insert(interface.clone()) {
                        all_interfaces.push(interface.clone());
                    }
                }
            }
        }

        all_interfaces
    }

    /// Check if a class implements a specific interface (direct or transitive)
    /// Pattern can be simple name or FQDN
    pub fn implements_interface(&self, class_fqdn: &str, interface_pattern: &str) -> bool {
        for interface in self.get_all_interfaces(class_fqdn) {
            if interface == interface_pattern || interface.ends_with(&format!(".{}", interface_pattern)) {
                return true;
            }
        }

        false
    }
}

impl Default for TypeResolver {
    fn default() -> Self {
        Self::new()
    }
}

/// Extract package declaration from AST
fn extract_package(tree: &Tree, source: &str) -> Option<String> {
    let packages = ast_explorer::find_nodes_by_kind(tree, "package_declaration");
    if packages.is_empty() {
        return None;
    }

    let package_node = packages[0];

    // Find scoped_identifier child
    for child in package_node.children(&mut package_node.walk()) {
        if child.kind() == "scoped_identifier" || child.kind() == "identifier" {
            let text = ast_explorer::node_text(child, source);
            return Some(text.to_string());
        }
    }

    None
}

/// Extract import declarations from AST
fn extract_imports(tree: &Tree, source: &str) -> (HashMap<String, String>, Vec<String>) {
    let mut explicit_imports = HashMap::new();
    let mut wildcard_imports = Vec::new();

    let imports = ast_explorer::find_nodes_by_kind(tree, "import_declaration");

    for import_node in imports {
        let mut has_asterisk = false;
        let mut import_path = String::new();

        // Check for asterisk (wildcard import)
        for child in import_node.children(&mut import_node.walk()) {
            match child.kind() {
                "asterisk" => has_asterisk = true,
                "scoped_identifier" | "identifier" => {
                    import_path = ast_explorer::node_text(child, source).to_string();
                }
                _ => {}
            }
        }

        if has_asterisk {
            // Wildcard import: java.util.*
            wildcard_imports.push(import_path);
        } else if !import_path.is_empty() {
            // Explicit import: java.util.List
            if let Some(simple_name) = import_path.split('.').next_back() {
                explicit_imports.insert(simple_name.to_string(), import_path);
            }
        }
    }

    (explicit_imports, wildcard_imports)
}

/// Extract class declarations from AST
fn extract_classes(
    tree: &Tree,
    source: &str,
    package_name: &Option<String>,
) -> Result<HashMap<String, ClassInfo>> {
    let mut classes = HashMap::new();

    let class_nodes = ast_explorer::find_nodes_by_kind(tree, "class_declaration");
    let interface_nodes = ast_explorer::find_nodes_by_kind(tree, "interface_declaration");
    let enum_nodes = ast_explorer::find_nodes_by_kind(tree, "enum_declaration");

    for class_node in class_nodes {
        let class_info = extract_class_info(class_node, source, package_name, false, false)?;
        classes.insert(class_info.simple_name.clone(), class_info);
    }

    for interface_node in interface_nodes {
        let class_info = extract_class_info(interface_node, source, package_name, true, false)?;
        classes.insert(class_info.simple_name.clone(), class_info);
    }

    for enum_node in enum_nodes {
        let class_info = extract_class_info(enum_node, source, package_name, false, true)?;
        classes.insert(class_info.simple_name.clone(), class_info);
    }

    Ok(classes)
}

/// Extract method calls (invocations) from AST
fn extract_method_calls(tree: &Tree, source: &str) -> Vec<MethodCall> {
    let mut method_calls = Vec::new();

    let invocation_nodes = ast_explorer::find_nodes_by_kind(tree, "method_invocation");

    for invocation_node in invocation_nodes {
        if let Some(method_call) = extract_method_call_info(invocation_node, source) {
            method_calls.push(method_call);
        }
    }

    method_calls
}

/// Extract constructor calls (new expressions) from AST
fn extract_constructor_calls(tree: &Tree, source: &str) -> Vec<ConstructorCall> {
    let mut constructor_calls = Vec::new();

    let creation_nodes = ast_explorer::find_nodes_by_kind(tree, "object_creation_expression");

    for creation_node in creation_nodes {
        if let Some(constructor_call) = extract_constructor_call_info(creation_node, source) {
            constructor_calls.push(constructor_call);
        }
    }

    constructor_calls
}

/// Extract information from a single object_creation_expression node
fn extract_constructor_call_info(creation_node: tree_sitter::Node, source: &str) -> Option<ConstructorCall> {
    let mut type_name = String::new();

    // Object creation expression structure:
    // object_creation_expression
    //   type: (type_identifier | generic_type)
    //   arguments: (argument_list)

    // Get type from "type" field
    if let Some(type_node) = creation_node.child_by_field_name("type") {
        type_name = extract_type_name_from_node(type_node, source);
    }

    // If still no type name, try to find first type_identifier child
    if type_name.is_empty() {
        for child in creation_node.children(&mut creation_node.walk()) {
            if child.kind() == "type_identifier" {
                type_name = ast_explorer::node_text(child, source).to_string();
                break;
            }
        }
    }

    if !type_name.is_empty() {
        Some(ConstructorCall {
            type_name: type_name.clone(),
            resolved_type: None,  // Will be resolved during query
            position: SourcePosition::from_node(creation_node),
        })
    } else {
        None
    }
}

/// Extract type name from a type node (handles generic types)
fn extract_type_name_from_node(type_node: tree_sitter::Node, source: &str) -> String {
    match type_node.kind() {
        "type_identifier" => {
            ast_explorer::node_text(type_node, source).to_string()
        }
        "generic_type" => {
            // For generic types like ArrayList<String>, extract just ArrayList
            for child in type_node.children(&mut type_node.walk()) {
                if child.kind() == "type_identifier" {
                    return ast_explorer::node_text(child, source).to_string();
                }
            }
            ast_explorer::node_text(type_node, source).to_string()
        }
        _ => {
            ast_explorer::node_text(type_node, source).to_string()
        }
    }
}

/// Extract annotations from AST
fn extract_annotations(tree: &Tree, source: &str, classes: &HashMap<String, ClassInfo>) -> Vec<AnnotationUsage> {
    let mut annotations = Vec::new();

    // Find all annotation nodes (marker_annotation and annotation)
    let marker_nodes = ast_explorer::find_nodes_by_kind(tree, "marker_annotation");
    let annotation_nodes = ast_explorer::find_nodes_by_kind(tree, "annotation");

    // Process marker annotations (e.g., @Override)
    for annotation_node in marker_nodes {
        if let Some(annotation) = extract_annotation_info(annotation_node, source, classes) {
            annotations.push(annotation);
        }
    }

    // Process annotations with parameters (e.g., @SuppressWarnings("unused"))
    for annotation_node in annotation_nodes {
        if let Some(annotation) = extract_annotation_info(annotation_node, source, classes) {
            annotations.push(annotation);
        }
    }

    annotations
}

/// Extract information from a single annotation node
fn extract_annotation_info(
    annotation_node: tree_sitter::Node,
    source: &str,
    classes: &HashMap<String, ClassInfo>,
) -> Option<AnnotationUsage> {
    let mut annotation_name = String::new();

    // Get annotation name from "name" field or first identifier
    if let Some(name_node) = annotation_node.child_by_field_name("name") {
        annotation_name = ast_explorer::node_text(name_node, source).to_string();
    } else {
        // Try to find first identifier or scoped_identifier child
        for child in annotation_node.children(&mut annotation_node.walk()) {
            if child.kind() == "identifier" || child.kind() == "scoped_identifier" || child.kind() == "type_identifier" {
                annotation_name = ast_explorer::node_text(child, source).to_string();
                break;
            }
        }
    }

    // Strip @ symbol if present
    annotation_name = annotation_name.trim_start_matches('@').to_string();

    if annotation_name.is_empty() {
        return None;
    }

    // Determine what is being annotated by looking at the parent/sibling nodes
    let target = determine_annotation_target(annotation_node, source, classes);

    Some(AnnotationUsage {
        annotation_name,
        target,
        position: SourcePosition::from_node(annotation_node),
    })
}

/// Extract detailed annotation information with elements from an annotation node
fn extract_detailed_annotation(
    annotation_node: tree_sitter::Node,
    source: &str,
) -> Option<AnnotationInfo> {
    let mut annotation_name = String::new();
    let mut elements = HashMap::new();

    // Get annotation name
    if let Some(name_node) = annotation_node.child_by_field_name("name") {
        annotation_name = ast_explorer::node_text(name_node, source).to_string();
    } else {
        // Try to find first identifier or scoped_identifier child
        for child in annotation_node.children(&mut annotation_node.walk()) {
            if child.kind() == "identifier" || child.kind() == "scoped_identifier" || child.kind() == "type_identifier" {
                annotation_name = ast_explorer::node_text(child, source).to_string();
                break;
            }
        }
    }

    // Strip @ symbol if present
    annotation_name = annotation_name.trim_start_matches('@').to_string();

    if annotation_name.is_empty() {
        return None;
    }

    // Extract annotation elements/arguments
    if let Some(arguments_node) = annotation_node.child_by_field_name("arguments") {
        elements = extract_annotation_elements(arguments_node, source);
    }

    Some(AnnotationInfo {
        name: annotation_name,
        fqdn: None,  // Will be resolved later
        elements,
        position: SourcePosition::from_node(annotation_node),
    })
}

/// Extract annotation elements (name-value pairs) from annotation arguments
fn extract_annotation_elements(
    arguments_node: tree_sitter::Node,
    source: &str,
) -> HashMap<String, String> {
    let mut elements = HashMap::new();

    for child in arguments_node.children(&mut arguments_node.walk()) {
        if child.kind() == "element_value_pair" {
            // Extract name and value from element_value_pair
            let mut name = String::new();
            let mut value = String::new();

            for element_child in child.children(&mut child.walk()) {
                match element_child.kind() {
                    "identifier" => {
                        if name.is_empty() {
                            name = ast_explorer::node_text(element_child, source).to_string();
                        }
                    }
                    "element_value_array_initializer" | "array_initializer" => {
                        // Handle array values like {... }
                        // Extract the first string literal from the array
                        for array_child in element_child.children(&mut element_child.walk()) {
                            if array_child.kind() == "string_literal" {
                                value = ast_explorer::node_text(array_child, source).to_string();
                                break;  // Take first element only
                            }
                        }
                    }
                    "string_literal" | "decimal_integer_literal" | "boolean_literal" => {
                        // Direct value (not an array)
                        value = ast_explorer::node_text(element_child, source).to_string();
                    }
                    _ if !element_child.kind().contains("(") && !element_child.kind().contains(")") && !element_child.kind().contains("=") => {
                        // This is likely the value (fallback)
                        if value.is_empty() {
                            value = ast_explorer::node_text(element_child, source).to_string();
                        }
                    }
                    _ => {}
                }
            }

            if !name.is_empty() && !value.is_empty() {
                // Clean up quotes from string values
                let cleaned_value = value.trim_matches('"').to_string();
                elements.insert(name, cleaned_value);
            }
        } else if child.kind() == "string_literal" || child.kind() == "identifier" || child.kind() == "decimal_integer_literal" || child.kind() == "boolean_literal" {
            // Single value annotation like @SuppressWarnings("unused")
            // Use "value" as the default key
            let value_text = ast_explorer::node_text(child, source);
            let cleaned_value = value_text.trim_matches('"').to_string();
            elements.insert("value".to_string(), cleaned_value);
        }
    }

    elements
}

/// Extract annotations from a node's children (looks for modifiers node containing annotations)
fn extract_annotations_from_node(
    parent_node: tree_sitter::Node,
    source: &str,
) -> Vec<AnnotationInfo> {
    let mut annotations = Vec::new();

    // Look through all children for annotations
    for child in parent_node.children(&mut parent_node.walk()) {
        match child.kind() {
            "marker_annotation" | "annotation" => {
                if let Some(annotation) = extract_detailed_annotation(child, source) {
                    annotations.push(annotation);
                }
            }
            "modifiers" => {
                // Modifiers node contains annotations
                for modifier_child in child.children(&mut child.walk()) {
                    if modifier_child.kind() == "marker_annotation" || modifier_child.kind() == "annotation" {
                        if let Some(annotation) = extract_detailed_annotation(modifier_child, source) {
                            annotations.push(annotation);
                        }
                    }
                }
            }
            _ => {}
        }
    }

    annotations
}

/// Determine what an annotation is targeting
fn determine_annotation_target(
    annotation_node: tree_sitter::Node,
    source: &str,
    classes: &HashMap<String, ClassInfo>,
) -> AnnotationTarget {
    // Walk up to find what this annotation is attached to
    let mut parent = annotation_node.parent();

    while let Some(parent_node) = parent {
        match parent_node.kind() {
            "class_declaration" | "interface_declaration" | "enum_declaration" => {
                // Annotation on a class
                if let Some(class_name) = find_class_name(parent_node, source) {
                    return AnnotationTarget::Class(class_name);
                }
            }
            "method_declaration" => {
                // Annotation on a method
                if let Some((class_name, method_name)) = find_method_context(parent_node, source, classes) {
                    return AnnotationTarget::Method(class_name, method_name);
                }
            }
            "field_declaration" => {
                // Annotation on a field
                if let Some((class_name, field_name)) = find_field_context(parent_node, source, classes) {
                    return AnnotationTarget::Field(class_name, field_name);
                }
            }
            "formal_parameter" => {
                // Annotation on a parameter
                if let Some((class_name, method_name, param_name)) = find_parameter_context(parent_node, source, classes) {
                    return AnnotationTarget::Parameter(class_name, method_name, param_name);
                }
            }
            _ => {}
        }
        parent = parent_node.parent();
    }

    AnnotationTarget::Unknown
}

/// Find class name from a class_declaration node
fn find_class_name(class_node: tree_sitter::Node, source: &str) -> Option<String> {
    for child in class_node.children(&mut class_node.walk()) {
        if child.kind() == "identifier" {
            return Some(ast_explorer::node_text(child, source).to_string());
        }
    }
    None
}

/// Find method context (class name and method name)
fn find_method_context(
    method_node: tree_sitter::Node,
    source: &str,
    classes: &HashMap<String, ClassInfo>,
) -> Option<(String, String)> {
    // Get method name
    let method_name = method_node.child_by_field_name("name")
        .map(|n| ast_explorer::node_text(n, source).to_string())?;

    // Find containing class
    let class_name = find_containing_class(method_node, source)?;

    Some((class_name, method_name))
}

/// Find field context (class name and field name)
fn find_field_context(
    field_node: tree_sitter::Node,
    source: &str,
    classes: &HashMap<String, ClassInfo>,
) -> Option<(String, String)> {
    // Get field name from variable_declarator
    let mut field_name = String::new();
    for child in field_node.children(&mut field_node.walk()) {
        if child.kind() == "variable_declarator" {
            for inner in child.children(&mut child.walk()) {
                if inner.kind() == "identifier" {
                    field_name = ast_explorer::node_text(inner, source).to_string();
                    break;
                }
            }
        }
    }

    if field_name.is_empty() {
        return None;
    }

    // Find containing class
    let class_name = find_containing_class(field_node, source)?;

    Some((class_name, field_name))
}

/// Find parameter context (class name, method name, and parameter name)
fn find_parameter_context(
    param_node: tree_sitter::Node,
    source: &str,
    classes: &HashMap<String, ClassInfo>,
) -> Option<(String, String, String)> {
    // Get parameter name
    let param_name = param_node.child_by_field_name("name")
        .map(|n| ast_explorer::node_text(n, source).to_string())?;

    // Find containing method
    let mut parent = param_node.parent();
    while let Some(p) = parent {
        if p.kind() == "method_declaration" {
            let (class_name, method_name) = find_method_context(p, source, classes)?;
            return Some((class_name, method_name, param_name));
        }
        parent = p.parent();
    }

    None
}

/// Find the containing class for a node
fn find_containing_class(node: tree_sitter::Node, source: &str) -> Option<String> {
    let mut parent = node.parent();
    while let Some(p) = parent {
        if p.kind() == "class_declaration" || p.kind() == "interface_declaration" || p.kind() == "enum_declaration" {
            return find_class_name(p, source);
        }
        parent = p.parent();
    }
    None
}

/// Extract local variable declarations from AST
fn extract_variables(tree: &Tree, source: &str, classes: &HashMap<String, ClassInfo>) -> Vec<VariableDeclaration> {
    let mut variables = Vec::new();

    // Find all local_variable_declaration nodes
    let var_nodes = ast_explorer::find_nodes_by_kind(tree, "local_variable_declaration");

    for var_node in var_nodes {
        if let Some(var_info) = extract_variable_info(var_node, source, classes) {
            variables.push(var_info);
        }
    }

    variables
}

/// Extract information from a single local_variable_declaration node
fn extract_variable_info(
    var_node: tree_sitter::Node,
    source: &str,
    classes: &HashMap<String, ClassInfo>,
) -> Option<VariableDeclaration> {
    // local_variable_declaration structure:
    //   type: (integral_type | type_identifier | generic_type)
    //   declarator: (variable_declarator)+
    //     identifier: variable name
    //     value: initializer expression (optional)

    // Extract type
    let type_name = var_node
        .child_by_field_name("type")
        .map(|node| extract_type_name_from_node(node, source))?;

    // Extract variable declarator (there can be multiple: int x = 1, y = 2;)
    // For now, we'll extract the first one
    let declarator = var_node
        .children(&mut var_node.walk())
        .find(|child| child.kind() == "variable_declarator")?;

    // Get variable name
    let variable_name = declarator
        .child_by_field_name("name")
        .map(|node| ast_explorer::node_text(node, source).to_string())?;

    // Find method context
    let method_context = find_method_name(var_node, source);

    // Find class context
    let class_context = find_containing_class(var_node, source);

    Some(VariableDeclaration {
        variable_name,
        type_name,
        resolved_type: None,  // Will be resolved during query
        method_context,
        class_context,
        position: SourcePosition::from_node(var_node),
    })
}

/// Find the method containing a given node
fn find_method_name(node: tree_sitter::Node, source: &str) -> Option<String> {
    let mut parent = node.parent();
    while let Some(p) = parent {
        if p.kind() == "method_declaration" || p.kind() == "constructor_declaration" {
            // Extract method/constructor name
            return p.child_by_field_name("name")
                .map(|n| ast_explorer::node_text(n, source).to_string());
        }
        parent = p.parent();
    }
    None
}

/// Extract information from a single method_invocation node
fn extract_method_call_info(invocation_node: tree_sitter::Node, source: &str) -> Option<MethodCall> {
    let mut method_name = String::new();
    let mut receiver_type: Option<String> = None;

    // Method invocation structure:
    // method_invocation
    //   object: (field_access | identifier)? - the receiver
    //   name: (identifier) - the method name
    //   arguments: (argument_list)

    // Get method name from the "name" field (this is most reliable)
    if let Some(name_node) = invocation_node.child_by_field_name("name") {
        method_name = ast_explorer::node_text(name_node, source).to_string();
    }

    // Get receiver from "object" field if present
    if let Some(object_node) = invocation_node.child_by_field_name("object") {
        receiver_type = Some(ast_explorer::node_text(object_node, source).to_string());
    }

    // If still no method name, try to find first identifier child
    if method_name.is_empty() {
        for child in invocation_node.children(&mut invocation_node.walk()) {
            if child.kind() == "identifier" {
                method_name = ast_explorer::node_text(child, source).to_string();
                break;
            }
        }
    }

    if !method_name.is_empty() {
        Some(MethodCall {
            method_name,
            receiver_type,
            position: SourcePosition::from_node(invocation_node),
        })
    } else {
        None
    }
}

/// Extract receiver type from field_access node
fn extract_receiver_from_field_access(field_access_node: tree_sitter::Node, source: &str) -> Option<String> {
    // For field_access, get the leftmost identifier
    for child in field_access_node.children(&mut field_access_node.walk()) {
        if child.kind() == "identifier" {
            return Some(ast_explorer::node_text(child, source).to_string());
        }
    }
    None
}

/// Extract information from a single class/interface/enum node
fn extract_class_info(
    class_node: tree_sitter::Node,
    source: &str,
    package_name: &Option<String>,
    is_interface: bool,
    is_enum: bool,
) -> Result<ClassInfo> {
    let mut simple_name = String::new();
    let mut extends = None;
    let mut implements = Vec::new();
    let mut methods = Vec::new();
    let mut fields = Vec::new();
    let annotations = extract_annotations_from_node(class_node, source);

    for child in class_node.children(&mut class_node.walk()) {
        match child.kind() {
            "identifier" => {
                if simple_name.is_empty() {
                    simple_name = ast_explorer::node_text(child, source).to_string();
                }
            }
            "superclass" => {
                extends = extract_type_from_superclass(child, source);
            }
            "super_interfaces" => {
                implements = extract_types_from_interfaces(child, source);
            }
            "class_body" | "interface_body" | "enum_body" => {
                for member in child.children(&mut child.walk()) {
                    match member.kind() {
                        "field_declaration" => {
                            if let Some(field) = extract_field_info(member, source) {
                                fields.push(field);
                            }
                        }
                        "method_declaration" => {
                            if let Some(method) = extract_method_info(member, source) {
                                methods.push(method);
                            }
                        }
                        "constructor_declaration" => {
                            // Constructors are special methods
                            if let Some(ctor) = extract_constructor_info(member, source, &simple_name) {
                                methods.push(ctor);
                            }
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }

    let fqdn = if let Some(pkg) = package_name {
        format!("{}.{}", pkg, simple_name)
    } else {
        simple_name.clone()
    };

    Ok(ClassInfo {
        simple_name,
        fqdn,
        extends,
        implements,
        methods,
        fields,
        annotations,
        is_interface,
        is_enum,
        position: SourcePosition::from_node(class_node),
    })
}

/// Extract superclass type from superclass node
fn extract_type_from_superclass(superclass_node: tree_sitter::Node, source: &str) -> Option<String> {
    for child in superclass_node.children(&mut superclass_node.walk()) {
        if child.kind() == "type_identifier" || child.kind() == "identifier" {
            return Some(ast_explorer::node_text(child, source).to_string());
        }
        if child.kind() == "generic_type" {
            // For generic types like List<String>, extract just List
            for inner in child.children(&mut child.walk()) {
                if inner.kind() == "type_identifier" || inner.kind() == "identifier" {
                    return Some(ast_explorer::node_text(inner, source).to_string());
                }
            }
        }
    }
    None
}

/// Extract interface types from super_interfaces node
fn extract_types_from_interfaces(interfaces_node: tree_sitter::Node, source: &str) -> Vec<String> {
    let mut types = Vec::new();

    for child in interfaces_node.children(&mut interfaces_node.walk()) {
        if child.kind() == "type_list" {
            for type_node in child.children(&mut child.walk()) {
                if type_node.kind() == "type_identifier" || type_node.kind() == "identifier" {
                    types.push(ast_explorer::node_text(type_node, source).to_string());
                } else if type_node.kind() == "generic_type" {
                    // Extract base type from generic
                    for inner in type_node.children(&mut type_node.walk()) {
                        if inner.kind() == "type_identifier" || inner.kind() == "identifier" {
                            types.push(ast_explorer::node_text(inner, source).to_string());
                            break;
                        }
                    }
                }
            }
        }
    }

    types
}

/// Extract field information from field_declaration node
fn extract_field_info(field_node: tree_sitter::Node, source: &str) -> Option<FieldInfo> {
    let mut type_name = String::new();
    let mut field_name = String::new();
    let annotations = extract_annotations_from_node(field_node, source);

    for child in field_node.children(&mut field_node.walk()) {
        match child.kind() {
            "integral_type" | "floating_point_type" | "boolean_type" | "type_identifier" | "identifier" => {
                if type_name.is_empty() {
                    type_name = ast_explorer::node_text(child, source).to_string();
                }
            }
            "generic_type" => {
                // Extract base type from generic
                for inner in child.children(&mut child.walk()) {
                    if inner.kind() == "type_identifier" || inner.kind() == "identifier" {
                        type_name = ast_explorer::node_text(inner, source).to_string();
                        break;
                    }
                }
            }
            "variable_declarator" => {
                for inner in child.children(&mut child.walk()) {
                    if inner.kind() == "identifier" {
                        field_name = ast_explorer::node_text(inner, source).to_string();
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    if !type_name.is_empty() && !field_name.is_empty() {
        Some(FieldInfo {
            name: field_name,
            type_name,
            annotations,
            position: SourcePosition::from_node(field_node),
        })
    } else {
        None
    }
}

/// Extract method information from method_declaration node
fn extract_method_info(method_node: tree_sitter::Node, source: &str) -> Option<MethodInfo> {
    let mut method_name = String::new();
    let mut return_type = String::new();
    let mut parameters = Vec::new();
    let annotations = extract_annotations_from_node(method_node, source);

    for child in method_node.children(&mut method_node.walk()) {
        match child.kind() {
            "identifier" => {
                if method_name.is_empty() {
                    method_name = ast_explorer::node_text(child, source).to_string();
                }
            }
            "integral_type" | "floating_point_type" | "boolean_type" | "type_identifier" | "void_type" => {
                if return_type.is_empty() {
                    return_type = ast_explorer::node_text(child, source).to_string();
                }
            }
            "generic_type" => {
                if return_type.is_empty() {
                    for inner in child.children(&mut child.walk()) {
                        if inner.kind() == "type_identifier" || inner.kind() == "identifier" {
                            return_type = ast_explorer::node_text(inner, source).to_string();
                            break;
                        }
                    }
                }
            }
            "formal_parameters" => {
                parameters = extract_parameters(child, source);
            }
            _ => {}
        }
    }

    if !method_name.is_empty() {
        Some(MethodInfo {
            name: method_name,
            return_type,
            parameters,
            annotations,
            position: SourcePosition::from_node(method_node),
        })
    } else {
        None
    }
}

/// Extract constructor information
fn extract_constructor_info(
    ctor_node: tree_sitter::Node,
    source: &str,
    class_name: &str,
) -> Option<MethodInfo> {
    let mut parameters = Vec::new();
    let annotations = extract_annotations_from_node(ctor_node, source);

    for child in ctor_node.children(&mut ctor_node.walk()) {
        if child.kind() == "formal_parameters" {
            parameters = extract_parameters(child, source);
        }
    }

    Some(MethodInfo {
        name: class_name.to_string(),  // Constructor name is the class name
        return_type: String::new(),     // Constructors have no return type
        parameters,
        annotations,
        position: SourcePosition::from_node(ctor_node),
    })
}

/// Extract parameters from formal_parameters node
fn extract_parameters(params_node: tree_sitter::Node, source: &str) -> Vec<(String, String)> {
    let mut parameters = Vec::new();

    for child in params_node.children(&mut params_node.walk()) {
        if child.kind() == "formal_parameter" {
            let mut param_type = String::new();
            let mut param_name = String::new();

            for param_child in child.children(&mut child.walk()) {
                match param_child.kind() {
                    "integral_type" | "floating_point_type" | "boolean_type" | "type_identifier" => {
                        if param_type.is_empty() {
                            param_type = ast_explorer::node_text(param_child, source).to_string();
                        }
                    }
                    "generic_type" => {
                        if param_type.is_empty() {
                            for inner in param_child.children(&mut param_child.walk()) {
                                if inner.kind() == "type_identifier" || inner.kind() == "identifier" {
                                    param_type = ast_explorer::node_text(inner, source).to_string();
                                    break;
                                }
                            }
                        }
                    }
                    "identifier" => {
                        if param_name.is_empty() {
                            param_name = ast_explorer::node_text(param_child, source).to_string();
                        }
                    }
                    _ => {}
                }
            }

            if !param_type.is_empty() && !param_name.is_empty() {
                parameters.push((param_name, param_type));
            }
        }
    }

    parameters
}

/// Check if a type name is a Java primitive
fn is_primitive(name: &str) -> bool {
    matches!(
        name,
        "boolean" | "byte" | "char" | "short" | "int" | "long" | "float" | "double" | "void"
    )
}

/// Check if a type is in java.lang package (implicit import)
fn is_java_lang_type(name: &str) -> bool {
    matches!(
        name,
        "String"
            | "Object"
            | "Class"
            | "Integer"
            | "Long"
            | "Double"
            | "Float"
            | "Boolean"
            | "Byte"
            | "Short"
            | "Character"
            | "Math"
            | "System"
            | "Thread"
            | "Runnable"
            | "Exception"
            | "RuntimeException"
            | "Error"
            | "Throwable"
            | "StringBuilder"
            | "StringBuffer"
            | "Number"
            | "Void"
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn test_package_extraction() {
        let fixture_path = PathBuf::from("tests/fixtures/Simple.java");
        if !fixture_path.exists() {
            eprintln!("Skipping test - fixture not found");
            return;
        }

        let file_info = TypeResolver::extract_file_info(&fixture_path).unwrap();
        assert_eq!(file_info.package_name, Some("com.example.simple".to_string()));
    }

    #[test]
    fn test_explicit_import_extraction() {
        let fixture_path = PathBuf::from("tests/fixtures/Simple.java");
        if !fixture_path.exists() {
            eprintln!("Skipping test - fixture not found");
            return;
        }

        let file_info = TypeResolver::extract_file_info(&fixture_path).unwrap();
        assert_eq!(
            file_info.explicit_imports.get("List"),
            Some(&"java.util.List".to_string())
        );
        assert_eq!(
            file_info.explicit_imports.get("ArrayList"),
            Some(&"java.util.ArrayList".to_string())
        );
    }

    #[test]
    fn test_class_extraction() {
        let fixture_path = PathBuf::from("tests/fixtures/Simple.java");
        if !fixture_path.exists() {
            eprintln!("Skipping test - fixture not found");
            return;
        }

        let file_info = TypeResolver::extract_file_info(&fixture_path).unwrap();
        let class_info = file_info.classes.get("Simple").unwrap();

        assert_eq!(class_info.simple_name, "Simple");
        assert_eq!(class_info.fqdn, "com.example.simple.Simple");
        assert_eq!(class_info.is_interface, false);
        assert_eq!(class_info.is_enum, false);
    }

    #[test]
    fn test_field_extraction() {
        let fixture_path = PathBuf::from("tests/fixtures/Simple.java");
        if !fixture_path.exists() {
            eprintln!("Skipping test - fixture not found");
            return;
        }

        let file_info = TypeResolver::extract_file_info(&fixture_path).unwrap();
        let class_info = file_info.classes.get("Simple").unwrap();

        assert_eq!(class_info.fields.len(), 2);

        let value_field = class_info.fields.iter().find(|f| f.name == "value").unwrap();
        assert_eq!(value_field.type_name, "int");

        let name_field = class_info.fields.iter().find(|f| f.name == "name").unwrap();
        assert_eq!(name_field.type_name, "String");
    }

    #[test]
    fn test_method_extraction() {
        let fixture_path = PathBuf::from("tests/fixtures/Simple.java");
        if !fixture_path.exists() {
            eprintln!("Skipping test - fixture not found");
            return;
        }

        let file_info = TypeResolver::extract_file_info(&fixture_path).unwrap();
        let class_info = file_info.classes.get("Simple").unwrap();

        // Check that methods were extracted
        assert!(class_info.methods.len() >= 5);

        // Check getValue method
        let get_value = class_info.methods.iter().find(|m| m.name == "getValue").unwrap();
        assert_eq!(get_value.return_type, "int");
        assert_eq!(get_value.parameters.len(), 0);

        // Check setValue method
        let set_value = class_info.methods.iter().find(|m| m.name == "setValue").unwrap();
        assert_eq!(set_value.return_type, "void");
        assert_eq!(set_value.parameters.len(), 1);
        assert_eq!(set_value.parameters[0].1, "int");

        // Check getItems method (generic return type)
        let get_items = class_info.methods.iter().find(|m| m.name == "getItems").unwrap();
        assert_eq!(get_items.return_type, "List");  // Base type extracted
    }

    #[test]
    fn test_extends_implements_extraction() {
        let fixture_path = PathBuf::from("tests/fixtures/InheritanceExample.java");
        if !fixture_path.exists() {
            eprintln!("Skipping test - fixture not found");
            return;
        }

        let file_info = TypeResolver::extract_file_info(&fixture_path).unwrap();
        let class_info = file_info.classes.get("InheritanceExample").unwrap();

        assert_eq!(class_info.extends, Some("BaseClass".to_string()));
        assert!(class_info.implements.contains(&"Runnable".to_string()));
        assert!(class_info.implements.contains(&"Serializable".to_string()));
    }

    #[test]
    fn test_resolve_primitive() {
        let mut resolver = TypeResolver::new();
        let file_path = PathBuf::from("tests/fixtures/Simple.java");

        if file_path.exists() {
            resolver.analyze_file(&file_path).unwrap();
        } else {
            // Create minimal FileInfo for test
            let file_info = FileInfo {
                file_path: file_path.clone(),
                package_name: Some("com.example".to_string()),
                explicit_imports: HashMap::new(),
                wildcard_imports: Vec::new(),
                classes: HashMap::new(),
                method_calls: vec![],
                constructor_calls: vec![],
                annotations: vec![],
                variables: vec![],
            };
            resolver.file_infos.insert(file_path.clone(), file_info);
        }

        let resolved = resolver.resolve_type_name("int", &file_path);
        assert_eq!(resolved, Some("int".to_string()));

        let resolved = resolver.resolve_type_name("boolean", &file_path);
        assert_eq!(resolved, Some("boolean".to_string()));
    }

    #[test]
    fn test_resolve_explicit_import() {
        let fixture_path = PathBuf::from("tests/fixtures/Simple.java");
        if !fixture_path.exists() {
            eprintln!("Skipping test - fixture not found");
            return;
        }

        let mut resolver = TypeResolver::new();
        resolver.analyze_file(&fixture_path).unwrap();

        let resolved = resolver.resolve_type_name("List", &fixture_path);
        assert_eq!(resolved, Some("java.util.List".to_string()));

        let resolved = resolver.resolve_type_name("ArrayList", &fixture_path);
        assert_eq!(resolved, Some("java.util.ArrayList".to_string()));
    }

    #[test]
    fn test_resolve_java_lang() {
        let mut resolver = TypeResolver::new();
        let file_path = PathBuf::from("tests/fixtures/Simple.java");

        if file_path.exists() {
            resolver.analyze_file(&file_path).unwrap();
        } else {
            let file_info = FileInfo {
                file_path: file_path.clone(),
                package_name: Some("com.example".to_string()),
                explicit_imports: HashMap::new(),
                wildcard_imports: Vec::new(),
                classes: HashMap::new(),
                method_calls: vec![],
                constructor_calls: vec![],
                annotations: vec![],
                variables: vec![],
            };
            resolver.file_infos.insert(file_path.clone(), file_info);
        }

        let resolved = resolver.resolve_type_name("String", &file_path);
        assert_eq!(resolved, Some("java.lang.String".to_string()));

        let resolved = resolver.resolve_type_name("Object", &file_path);
        assert_eq!(resolved, Some("java.lang.Object".to_string()));
    }

    #[test]
    fn test_resolve_same_package() {
        let fixture_path = PathBuf::from("tests/fixtures/Simple.java");
        if !fixture_path.exists() {
            eprintln!("Skipping test - fixture not found");
            return;
        }

        let mut resolver = TypeResolver::new();
        resolver.analyze_file(&fixture_path).unwrap();

        // "Simple" should resolve to same package
        let resolved = resolver.resolve_type_name("Simple", &fixture_path);
        assert_eq!(resolved, Some("com.example.simple.Simple".to_string()));
    }

    #[test]
    fn test_default_package() {
        let source = r#"
            public class DefaultPackageClass {
                private int value;
            }
        "#;

        let tree = language_config::parse_source(source).unwrap();
        let package_name = extract_package(&tree, source);
        assert_eq!(package_name, None);

        // Test class extraction with no package
        let classes = extract_classes(&tree, source, &None).unwrap();
        let class_info = classes.get("DefaultPackageClass").unwrap();
        assert_eq!(class_info.fqdn, "DefaultPackageClass");
    }

    #[test]
    fn test_wildcard_import_extraction() {
        let source = r#"
            package com.example;

            import java.util.*;
            import java.io.*;

            public class WildcardTest {
            }
        "#;

        let tree = language_config::parse_source(source).unwrap();
        let (explicit, wildcard) = extract_imports(&tree, source);

        assert!(explicit.is_empty());
        assert!(wildcard.contains(&"java.util".to_string()));
        assert!(wildcard.contains(&"java.io".to_string()));
    }

    #[test]
    fn test_global_index_build() {
        let fixture_path = PathBuf::from("tests/fixtures/Simple.java");
        if !fixture_path.exists() {
            eprintln!("Skipping test - fixture not found");
            return;
        }

        let mut resolver = TypeResolver::new();
        resolver.analyze_file(&fixture_path).unwrap();
        resolver.build_global_index();

        // Check that Simple is in the index
        let fqdns = resolver.global_type_index.get("Simple").unwrap();
        assert!(fqdns.contains(&"com.example.simple.Simple".to_string()));
    }

    #[test]
    fn test_is_primitive() {
        assert!(is_primitive("int"));
        assert!(is_primitive("boolean"));
        assert!(is_primitive("char"));
        assert!(is_primitive("double"));
        assert!(is_primitive("void"));

        assert!(!is_primitive("String"));
        assert!(!is_primitive("List"));
        assert!(!is_primitive("Integer"));
    }

    #[test]
    fn test_is_java_lang_type() {
        assert!(is_java_lang_type("String"));
        assert!(is_java_lang_type("Object"));
        assert!(is_java_lang_type("Integer"));
        assert!(is_java_lang_type("Exception"));

        assert!(!is_java_lang_type("List"));
        assert!(!is_java_lang_type("ArrayList"));
        assert!(!is_java_lang_type("Custom"));
    }

    // ========== Inheritance Tracking Tests ==========

    #[test]
    fn test_build_inheritance_maps() {
        let fixture_path = PathBuf::from("tests/fixtures/InheritanceExample.java");
        if !fixture_path.exists() {
            eprintln!("Skipping test - fixture not found");
            return;
        }

        let mut resolver = TypeResolver::new();
        resolver.analyze_file(&fixture_path).unwrap();
        resolver.build_global_index();
        resolver.build_inheritance_maps();

        // InheritanceExample extends BaseClass
        let class_fqdn = "com.example.inheritance.InheritanceExample";
        let parent = resolver.get_parent_class(class_fqdn);
        assert!(parent.is_some());
        // Note: parent will be resolved based on imports in the file
    }

    #[test]
    fn test_extends_class_direct() {
        // Create a simple inheritance hierarchy
        let mut resolver = TypeResolver::new();

        // Manually create file info with inheritance
        use std::collections::HashMap;
        let file1 = PathBuf::from("test1.java");
        let mut classes1 = HashMap::new();

        // Parent class
        classes1.insert(
            "Parent".to_string(),
            ClassInfo {
                simple_name: "Parent".to_string(),
                fqdn: "com.test.Parent".to_string(),
                extends: None,
                implements: vec![],
                methods: vec![],
                fields: vec![],
                annotations: vec![],
                is_interface: false,
                is_enum: false,
                position: SourcePosition::unknown(),
            },
        );

        // Child class that extends Parent
        classes1.insert(
            "Child".to_string(),
            ClassInfo {
                simple_name: "Child".to_string(),
                fqdn: "com.test.Child".to_string(),
                extends: Some("Parent".to_string()),
                implements: vec![],
                methods: vec![],
                fields: vec![],
                annotations: vec![],
                is_interface: false,
                is_enum: false,
                position: SourcePosition::unknown(),
            },
        );

        let file_info1 = FileInfo {
            file_path: file1.clone(),
            package_name: Some("com.test".to_string()),
            explicit_imports: HashMap::new(),
            wildcard_imports: vec![],
            classes: classes1,
            method_calls: vec![],
            constructor_calls: vec![],
            annotations: vec![],
            variables: vec![],
        };

        resolver.file_infos.insert(file1, file_info1);
        resolver.build_global_index();
        resolver.build_inheritance_maps();

        // Test direct inheritance
        assert!(resolver.extends_class("com.test.Child", "com.test.Parent"));
        assert!(resolver.extends_class("com.test.Child", "Parent"));
        assert!(!resolver.extends_class("com.test.Parent", "Child"));
    }

    #[test]
    fn test_extends_class_transitive() {
        // Create a transitive inheritance hierarchy: GrandChild -> Child -> Parent
        let mut resolver = TypeResolver::new();
        use std::collections::HashMap;

        let file1 = PathBuf::from("test.java");
        let mut classes = HashMap::new();

        classes.insert(
            "Parent".to_string(),
            ClassInfo {
                simple_name: "Parent".to_string(),
                fqdn: "com.test.Parent".to_string(),
                extends: None,
                implements: vec![],
                methods: vec![],
                fields: vec![],
                annotations: vec![],
                is_interface: false,
                is_enum: false,
                position: SourcePosition::unknown(),
            },
        );

        classes.insert(
            "Child".to_string(),
            ClassInfo {
                simple_name: "Child".to_string(),
                fqdn: "com.test.Child".to_string(),
                extends: Some("Parent".to_string()),
                implements: vec![],
                methods: vec![],
                fields: vec![],
                annotations: vec![],
                is_interface: false,
                is_enum: false,
                position: SourcePosition::unknown(),
            },
        );

        classes.insert(
            "GrandChild".to_string(),
            ClassInfo {
                simple_name: "GrandChild".to_string(),
                fqdn: "com.test.GrandChild".to_string(),
                extends: Some("Child".to_string()),
                implements: vec![],
                methods: vec![],
                fields: vec![],
                annotations: vec![],
                is_interface: false,
                is_enum: false,
                position: SourcePosition::unknown(),
            },
        );

        let file_info = FileInfo {
            file_path: file1.clone(),
            package_name: Some("com.test".to_string()),
            explicit_imports: HashMap::new(),
            wildcard_imports: vec![],
            classes,
            method_calls: vec![],
            constructor_calls: vec![],
            annotations: vec![],
            variables: vec![],
        };

        resolver.file_infos.insert(file1, file_info);
        resolver.build_global_index();
        resolver.build_inheritance_maps();

        // Test transitive inheritance
        assert!(resolver.extends_class("com.test.GrandChild", "com.test.Child"));
        assert!(resolver.extends_class("com.test.GrandChild", "com.test.Parent"));
        assert!(resolver.extends_class("com.test.GrandChild", "Child"));
        assert!(resolver.extends_class("com.test.GrandChild", "Parent"));

        assert!(resolver.extends_class("com.test.Child", "com.test.Parent"));
        assert!(!resolver.extends_class("com.test.Parent", "com.test.Child"));
    }

    #[test]
    fn test_get_all_parents() {
        // Create hierarchy: GrandChild -> Child -> Parent
        let mut resolver = TypeResolver::new();
        use std::collections::HashMap;

        let file1 = PathBuf::from("test.java");
        let mut classes = HashMap::new();

        classes.insert(
            "Parent".to_string(),
            ClassInfo {
                simple_name: "Parent".to_string(),
                fqdn: "com.test.Parent".to_string(),
                extends: None,
                implements: vec![],
                methods: vec![],
                fields: vec![],
                annotations: vec![],
                is_interface: false,
                is_enum: false,
                position: SourcePosition::unknown(),
            },
        );

        classes.insert(
            "Child".to_string(),
            ClassInfo {
                simple_name: "Child".to_string(),
                fqdn: "com.test.Child".to_string(),
                extends: Some("Parent".to_string()),
                implements: vec![],
                methods: vec![],
                fields: vec![],
                annotations: vec![],
                is_interface: false,
                is_enum: false,
                position: SourcePosition::unknown(),
            },
        );

        classes.insert(
            "GrandChild".to_string(),
            ClassInfo {
                simple_name: "GrandChild".to_string(),
                fqdn: "com.test.GrandChild".to_string(),
                extends: Some("Child".to_string()),
                implements: vec![],
                methods: vec![],
                fields: vec![],
                annotations: vec![],
                is_interface: false,
                is_enum: false,
                position: SourcePosition::unknown(),
            },
        );

        let file_info = FileInfo {
            file_path: file1.clone(),
            package_name: Some("com.test".to_string()),
            explicit_imports: HashMap::new(),
            wildcard_imports: vec![],
            classes,
            method_calls: vec![],
            constructor_calls: vec![],
            annotations: vec![],
            variables: vec![],
        };

        resolver.file_infos.insert(file1, file_info);
        resolver.build_global_index();
        resolver.build_inheritance_maps();

        let parents = resolver.get_all_parents("com.test.GrandChild");
        assert_eq!(parents.len(), 2);
        assert_eq!(parents[0], "com.test.Child");
        assert_eq!(parents[1], "com.test.Parent");

        let parents = resolver.get_all_parents("com.test.Child");
        assert_eq!(parents.len(), 1);
        assert_eq!(parents[0], "com.test.Parent");

        let parents = resolver.get_all_parents("com.test.Parent");
        assert_eq!(parents.len(), 0);
    }

    #[test]
    fn test_implements_interface_direct() {
        let mut resolver = TypeResolver::new();
        use std::collections::HashMap;

        let file1 = PathBuf::from("test.java");
        let mut classes = HashMap::new();

        classes.insert(
            "MyClass".to_string(),
            ClassInfo {
                simple_name: "MyClass".to_string(),
                fqdn: "com.test.MyClass".to_string(),
                extends: None,
                implements: vec!["Runnable".to_string(), "Cloneable".to_string()],
                methods: vec![],
                fields: vec![],
                annotations: vec![],
                is_interface: false,
                is_enum: false,
                position: SourcePosition::unknown(),
            },
        );

        let mut explicit_imports = HashMap::new();
        explicit_imports.insert("Runnable".to_string(), "java.lang.Runnable".to_string());
        explicit_imports.insert("Cloneable".to_string(), "java.lang.Cloneable".to_string());

        let file_info = FileInfo {
            file_path: file1.clone(),
            package_name: Some("com.test".to_string()),
            explicit_imports,
            wildcard_imports: vec![],
            classes,
            method_calls: vec![],
            constructor_calls: vec![],
            annotations: vec![],
            variables: vec![],
        };

        resolver.file_infos.insert(file1, file_info);
        resolver.build_global_index();
        resolver.build_inheritance_maps();

        // Test direct interface implementation
        assert!(resolver.implements_interface("com.test.MyClass", "java.lang.Runnable"));
        assert!(resolver.implements_interface("com.test.MyClass", "Runnable"));
        assert!(resolver.implements_interface("com.test.MyClass", "java.lang.Cloneable"));
        assert!(resolver.implements_interface("com.test.MyClass", "Cloneable"));
        assert!(!resolver.implements_interface("com.test.MyClass", "Serializable"));
    }

    #[test]
    fn test_implements_interface_transitive() {
        // Create hierarchy: Child extends Parent, Parent implements Runnable
        let mut resolver = TypeResolver::new();
        use std::collections::HashMap;

        let file1 = PathBuf::from("test.java");
        let mut classes = HashMap::new();

        classes.insert(
            "Parent".to_string(),
            ClassInfo {
                simple_name: "Parent".to_string(),
                fqdn: "com.test.Parent".to_string(),
                extends: None,
                implements: vec!["Runnable".to_string()],
                methods: vec![],
                fields: vec![],
                annotations: vec![],
                is_interface: false,
                is_enum: false,
                position: SourcePosition::unknown(),
            },
        );

        classes.insert(
            "Child".to_string(),
            ClassInfo {
                simple_name: "Child".to_string(),
                fqdn: "com.test.Child".to_string(),
                extends: Some("Parent".to_string()),
                implements: vec!["Cloneable".to_string()],
                methods: vec![],
                fields: vec![],
                annotations: vec![],
                is_interface: false,
                is_enum: false,
                position: SourcePosition::unknown(),
            },
        );

        let mut explicit_imports = HashMap::new();
        explicit_imports.insert("Runnable".to_string(), "java.lang.Runnable".to_string());
        explicit_imports.insert("Cloneable".to_string(), "java.lang.Cloneable".to_string());

        let file_info = FileInfo {
            file_path: file1.clone(),
            package_name: Some("com.test".to_string()),
            explicit_imports,
            wildcard_imports: vec![],
            classes,
            method_calls: vec![],
            constructor_calls: vec![],
            annotations: vec![],
            variables: vec![],
        };

        resolver.file_infos.insert(file1, file_info);
        resolver.build_global_index();
        resolver.build_inheritance_maps();

        // Child should implement both its own interface and parent's interface
        assert!(resolver.implements_interface("com.test.Child", "java.lang.Cloneable"));
        assert!(resolver.implements_interface("com.test.Child", "Cloneable"));
        assert!(resolver.implements_interface("com.test.Child", "java.lang.Runnable"));
        assert!(resolver.implements_interface("com.test.Child", "Runnable"));

        // Parent should only implement its own interface
        assert!(resolver.implements_interface("com.test.Parent", "java.lang.Runnable"));
        assert!(!resolver.implements_interface("com.test.Parent", "Cloneable"));
    }

    #[test]
    fn test_get_all_interfaces() {
        let mut resolver = TypeResolver::new();
        use std::collections::HashMap;

        let file1 = PathBuf::from("test.java");
        let mut classes = HashMap::new();

        classes.insert(
            "Parent".to_string(),
            ClassInfo {
                simple_name: "Parent".to_string(),
                fqdn: "com.test.Parent".to_string(),
                extends: None,
                implements: vec!["Runnable".to_string()],
                methods: vec![],
                fields: vec![],
                annotations: vec![],
                is_interface: false,
                is_enum: false,
                position: SourcePosition::unknown(),
            },
        );

        classes.insert(
            "Child".to_string(),
            ClassInfo {
                simple_name: "Child".to_string(),
                fqdn: "com.test.Child".to_string(),
                extends: Some("Parent".to_string()),
                implements: vec!["Cloneable".to_string(), "Serializable".to_string()],
                methods: vec![],
                fields: vec![],
                annotations: vec![],
                is_interface: false,
                is_enum: false,
                position: SourcePosition::unknown(),
            },
        );

        let mut explicit_imports = HashMap::new();
        explicit_imports.insert("Runnable".to_string(), "java.lang.Runnable".to_string());
        explicit_imports.insert("Cloneable".to_string(), "java.lang.Cloneable".to_string());
        explicit_imports.insert("Serializable".to_string(), "java.io.Serializable".to_string());

        let file_info = FileInfo {
            file_path: file1.clone(),
            package_name: Some("com.test".to_string()),
            explicit_imports,
            wildcard_imports: vec![],
            classes,
            method_calls: vec![],
            constructor_calls: vec![],
            annotations: vec![],
            variables: vec![],
        };

        resolver.file_infos.insert(file1, file_info);
        resolver.build_global_index();
        resolver.build_inheritance_maps();

        let interfaces = resolver.get_all_interfaces("com.test.Child");
        assert_eq!(interfaces.len(), 3);
        assert!(interfaces.contains(&"java.lang.Cloneable".to_string()));
        assert!(interfaces.contains(&"java.io.Serializable".to_string()));
        assert!(interfaces.contains(&"java.lang.Runnable".to_string()));
    }
}

#[cfg(test)]
mod annotation_tests {
    use super::*;
    use crate::java_graph::language_config;

    #[test]
    fn test_annotation_with_array_element() {
        let java_code = r#"
package io.konveyor.demo.ordermanagement.config;

import org.springframework.data.jpa.repository.config.EnableJpaRepositories;
import org.springframework.context.annotation.Bean;

@EnableJpaRepositories(basePackages = {
        "io.konveyor.demo.ordermanagement.repository"
})
public class PersistenceConfig {
    @Bean
    public void entityManagerFactory() {}
}
"#;

        // Parse the code
        let tree = language_config::parse_source(java_code).unwrap();
        let source = java_code;
        
        // Find class_declaration node
        let class_nodes = ast_explorer::find_nodes_by_kind(&tree, "class_declaration");
        assert!(!class_nodes.is_empty(), "Should find at least one class");
        
        let class_node = class_nodes[0];
        
        // Extract annotations from the class
        let annotations = extract_annotations_from_node(class_node, &source);
        
        println!("Found {} annotations", annotations.len());
        for ann in &annotations {
            println!("Annotation: {} (FQDN: {:?})", ann.name, ann.fqdn);
            println!("  Elements: {:?}", ann.elements);
        }
        
        // Should have @EnableJpaRepositories annotation
        let enable_jpa = annotations.iter()
            .find(|a| a.name == "EnableJpaRepositories")
            .expect("Should find EnableJpaRepositories annotation");
        
        // Check if basePackages element was extracted
        assert!(enable_jpa.elements.contains_key("basePackages"), 
            "Should have basePackages element");
        
        let base_packages = &enable_jpa.elements["basePackages"];
        assert_eq!(base_packages, "io.konveyor.demo.ordermanagement.repository",
            "basePackages should match");
    }
}
