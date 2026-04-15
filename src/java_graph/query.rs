// Query engine for location types

use anyhow::{Context, Result};
use regex::Regex;
use stack_graphs::graph::StackGraph;
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};

use super::type_resolver::TypeResolver;

/// All 15 location types supported by the Java analyzer
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LocationType {
    // Simple types (filter by syntax_type)
    Type,                // class, interface, enum declarations
    Import,              // import statements
    Package,             // package declarations
    Variable,            // variable declarations
    Field,               // field declarations
    Method,              // method declarations
    Class,               // class declarations specifically
    Enum,                // enum declarations

    // Semantic types (require TypeResolver)
    Inheritance,         // extends clauses
    ImplementsType,      // implements clauses

    // Call sites
    MethodCall,          // method invocations
    ConstructorCall,     // new expressions

    // Other
    Annotation,          // annotations
    ReturnType,          // method return types
}

/// Pattern options for controlling matching behavior
#[derive(Debug, Clone)]
pub struct PatternOptions {
    /// Case-insensitive matching
    pub case_insensitive: bool,
    /// Match full word only (no partial matches)
    pub whole_word: bool,
}

impl Default for PatternOptions {
    fn default() -> Self {
        PatternOptions {
            case_insensitive: false,
            whole_word: false,
        }
    }
}

/// Pattern for matching symbols
#[derive(Debug, Clone)]
pub enum Pattern {
    /// Exact string match
    Literal(String, PatternOptions),
    /// Wildcard pattern (e.g., "org.springframework.*")
    Wildcard(String, PatternOptions),
    /// Regular expression
    Regex(Regex, PatternOptions),
    /// Composite pattern combining multiple patterns
    Composite(CompositePattern),
}

/// Composite pattern logic for combining patterns
#[derive(Debug, Clone)]
pub enum CompositePattern {
    /// All patterns must match (AND)
    And(Vec<Pattern>),
    /// Any pattern must match (OR)
    Or(Vec<Pattern>),
    /// Pattern must not match (NOT)
    Not(Box<Pattern>),
}

impl Pattern {
    /// Create a pattern from a string, detecting the type
    pub fn from_string(s: &str) -> Result<Self> {
        Self::from_string_with_options(s, PatternOptions::default())
    }

    /// Create a pattern from a string with custom options
    pub fn from_string_with_options(s: &str, options: PatternOptions) -> Result<Self> {
        // Check for regex metacharacters that indicate this is a regex, not a wildcard
        let regex_indicators = ['^', '$', '|', '+', '?', '[', '(', '{', '\\'];
        let has_regex_char = regex_indicators.iter().any(|&c| s.contains(c));

        // Check for .* which is regex, not wildcard
        let has_dot_star = s.contains(".*");

        if has_regex_char || has_dot_star {
            // Regex pattern
            let regex_pattern = if options.case_insensitive {
                format!("(?i){}", s)
            } else {
                s.to_string()
            };

            let regex = Regex::new(&regex_pattern)
                .with_context(|| format!("Invalid regex pattern: {}", s))?;
            Ok(Pattern::Regex(regex, options))
        } else if s.contains('*') {
            // Wildcard pattern (e.g., "org.springframework.*")
            Ok(Pattern::Wildcard(s.to_string(), options))
        } else {
            // Literal pattern
            Ok(Pattern::Literal(s.to_string(), options))
        }
    }

    /// Create a case-insensitive pattern
    pub fn from_string_case_insensitive(s: &str) -> Result<Self> {
        Self::from_string_with_options(s, PatternOptions {
            case_insensitive: true,
            whole_word: false,
        })
    }

    /// Create an AND composite pattern
    pub fn and(patterns: Vec<Pattern>) -> Self {
        Pattern::Composite(CompositePattern::And(patterns))
    }

    /// Create an OR composite pattern
    pub fn or(patterns: Vec<Pattern>) -> Self {
        Pattern::Composite(CompositePattern::Or(patterns))
    }

    /// Create a NOT pattern
    pub fn not(pattern: Pattern) -> Self {
        Pattern::Composite(CompositePattern::Not(Box::new(pattern)))
    }

    /// Check if a value matches this pattern
    pub fn matches(&self, value: &str) -> bool {
        match self {
            Pattern::Literal(literal, options) => {
                let matches = if options.case_insensitive {
                    literal.to_lowercase() == value.to_lowercase()
                } else {
                    literal == value
                };

                if options.whole_word && matches {
                    // For whole word, check that value is the entire string
                    literal.len() == value.len()
                } else {
                    matches
                }
            }
            Pattern::Wildcard(wildcard, options) => {
                let pattern_to_match = if options.case_insensitive {
                    wildcard.to_lowercase()
                } else {
                    wildcard.clone()
                };
                let value_to_match = if options.case_insensitive {
                    value.to_lowercase()
                } else {
                    value.to_string()
                };
                wildmatch::WildMatch::new(&pattern_to_match).matches(&value_to_match)
            }
            Pattern::Regex(regex, _options) => {
                // Options already applied during regex compilation
                regex.is_match(value)
            }
            Pattern::Composite(composite) => composite.matches(value),
        }
    }
}

impl CompositePattern {
    /// Check if value matches this composite pattern
    pub fn matches(&self, value: &str) -> bool {
        match self {
            CompositePattern::And(patterns) => {
                patterns.iter().all(|p| p.matches(value))
            }
            CompositePattern::Or(patterns) => {
                patterns.iter().any(|p| p.matches(value))
            }
            CompositePattern::Not(pattern) => {
                !pattern.matches(value)
            }
        }
    }
}

/// Pattern cache for performance optimization
pub struct PatternCache {
    compiled_patterns: Arc<Mutex<HashMap<String, Pattern>>>,
}

impl PatternCache {
    /// Create a new pattern cache
    pub fn new() -> Self {
        PatternCache {
            compiled_patterns: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    /// Get or compile a pattern
    pub fn get_or_compile(&self, pattern_str: &str) -> Result<Pattern> {
        let mut cache = self.compiled_patterns.lock().unwrap();

        if let Some(pattern) = cache.get(pattern_str) {
            Ok(pattern.clone())
        } else {
            let pattern = Pattern::from_string(pattern_str)?;
            cache.insert(pattern_str.to_string(), pattern.clone());
            Ok(pattern)
        }
    }

    /// Clear the cache
    pub fn clear(&self) {
        let mut cache = self.compiled_patterns.lock().unwrap();
        cache.clear();
    }

    /// Get cache size
    pub fn size(&self) -> usize {
        let cache = self.compiled_patterns.lock().unwrap();
        cache.len()
    }
}

impl Default for PatternCache {
    fn default() -> Self {
        Self::new()
    }
}

/// Access modifier filter
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum AccessModifier {
    Public,
    Protected,
    Private,
    Package,  // Default/package-private
}

/// Query filters for advanced filtering
#[derive(Debug, Clone, Default)]
pub struct QueryFilters {
    /// Optional annotation filter (match elements with this annotation)
    pub annotated: Option<String>,
    /// Optional access modifier filter
    pub access_modifier: Option<AccessModifier>,
    /// Optional static filter (true = only static, false = only non-static, None = both)
    pub is_static: Option<bool>,
    /// Optional final filter (true = only final, false = only non-final, None = both)
    pub is_final: Option<bool>,
    /// Optional abstract filter (true = only abstract, false = only non-abstract, None = both)
    pub is_abstract: Option<bool>,
    /// Exclude test files/classes
    pub exclude_tests: bool,
    /// Include only deprecated elements
    pub deprecated_only: bool,
}

/// Query parameters for referenced capability
#[derive(Debug, Clone)]
pub struct ReferencedQuery {
    pub pattern: Pattern,
    pub location: LocationType,
    pub annotated: Option<String>,  // Optional annotation filter (deprecated, use filters instead)
    pub filters: Option<QueryFilters>,  // Advanced filters
}

/// Result of a query - represents a match
#[derive(Debug, Clone)]
pub struct QueryResult {
    pub file_path: String,
    pub line_number: usize,
    pub column: usize,
    pub symbol: String,
    pub fqdn: Option<String>,
}

/// Query engine that combines stack-graph and TypeResolver
pub struct QueryEngine {
    graph: StackGraph,
    type_resolver: TypeResolver,
    pattern_cache: PatternCache,
}

impl QueryEngine {
    /// Create a new query engine
    pub fn new(graph: StackGraph, type_resolver: TypeResolver) -> Self {
        QueryEngine {
            graph,
            type_resolver,
            pattern_cache: PatternCache::new(),
        }
    }

    /// Execute a referenced query
    pub fn query(&self, query: &ReferencedQuery) -> Result<Vec<QueryResult>> {
        // Execute the query based on location type
        let mut results = match query.location {
            LocationType::Import => self.query_imports(&query.pattern),
            LocationType::Package => self.query_packages(&query.pattern),
            LocationType::Class => self.query_classes(&query.pattern),
            LocationType::Type => self.query_types(&query.pattern),
            LocationType::Field => self.query_fields(&query.pattern),
            LocationType::Method => self.query_methods(&query.pattern),
            LocationType::Enum => self.query_enums(&query.pattern),
            LocationType::Inheritance => self.query_inheritance(&query.pattern),
            LocationType::ImplementsType => self.query_implements(&query.pattern),
            LocationType::MethodCall => self.query_method_calls(&query.pattern),
            LocationType::ConstructorCall => self.query_constructor_calls(&query.pattern),
            LocationType::Annotation => self.query_annotations(&query.pattern),
            LocationType::Variable => self.query_variables(&query.pattern),
            LocationType::ReturnType => self.query_return_types(&query.pattern),
        }?;

        // Apply filters if provided
        if let Some(ref filters) = query.filters {
            results = self.apply_filters(results, filters);
        }

        Ok(results)
    }

    /// Apply advanced filters to query results
    fn apply_filters(&self, results: Vec<QueryResult>, filters: &QueryFilters) -> Vec<QueryResult> {
        results.into_iter().filter(|result| {
            // Annotation filter
            if let Some(ref annotation) = filters.annotated {
                // Check if the symbol has the specified annotation
                // This would require additional metadata in QueryResult
                // For now, we'll keep the result if annotated filter is specified
                // A complete implementation would need annotation metadata per result
            }

            // Exclude tests filter
            if filters.exclude_tests {
                if result.file_path.contains("/test/") ||
                   result.file_path.contains("Test.java") ||
                   result.symbol.contains("Test") {
                    return false;
                }
            }

            // More filters can be applied here based on available metadata
            true
        }).collect()
    }

    /// Get pattern cache (for statistics/debugging)
    pub fn pattern_cache_size(&self) -> usize {
        self.pattern_cache.size()
    }

    /// Clear pattern cache
    pub fn clear_pattern_cache(&self) {
        self.pattern_cache.clear();
    }

    /// Query for import statements
    fn query_imports(&self, pattern: &Pattern) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();

        // Use TypeResolver's file_infos to find imports
        for (file_path, file_info) in &self.type_resolver.file_infos {
            for (simple_name, fqdn) in &file_info.explicit_imports {
                if pattern.matches(fqdn) || pattern.matches(simple_name) {
                    results.push(QueryResult {
                        file_path: file_path.display().to_string(),
                        line_number: 0,  // TODO: Extract from AST
                        column: 0,
                        symbol: simple_name.clone(),
                        fqdn: Some(fqdn.clone()),
                    });
                }
            }

            // Also check wildcard imports
            for wildcard_pkg in &file_info.wildcard_imports {
                if pattern.matches(wildcard_pkg) {
                    results.push(QueryResult {
                        file_path: file_path.display().to_string(),
                        line_number: 0,  // TODO: Extract from AST
                        column: 0,
                        symbol: format!("{}.*", wildcard_pkg),
                        fqdn: Some(wildcard_pkg.clone()),
                    });
                }
            }
        }

        Ok(results)
    }

    /// Query for package declarations
    fn query_packages(&self, pattern: &Pattern) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();

        for (file_path, file_info) in &self.type_resolver.file_infos {
            if let Some(package_name) = &file_info.package_name {
                if pattern.matches(package_name) {
                    results.push(QueryResult {
                        file_path: file_path.display().to_string(),
                        line_number: 0,  // TODO: Extract from AST
                        column: 0,
                        symbol: package_name.clone(),
                        fqdn: Some(package_name.clone()),
                    });
                }
            }
        }

        Ok(results)
    }

    /// Query for class declarations
    fn query_classes(&self, pattern: &Pattern) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();

        // Use TypeResolver to get all classes
        for (file_path, file_info) in &self.type_resolver.file_infos {
            for class_info in file_info.classes.values() {
                if !class_info.is_interface && !class_info.is_enum {
                    let fqdn = &class_info.fqdn;

                    if pattern.matches(fqdn) || pattern.matches(&class_info.simple_name) {
                        results.push(QueryResult {
                            file_path: file_path.display().to_string(),
                            line_number: class_info.position.line,
                            column: class_info.position.column,
                            symbol: class_info.simple_name.clone(),
                            fqdn: Some(fqdn.clone()),
                        });
                    }
                }
            }
        }

        Ok(results)
    }

    /// Query for type declarations (class, interface, enum)
    fn query_types(&self, pattern: &Pattern) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();

        for (file_path, file_info) in &self.type_resolver.file_infos {
            for class_info in file_info.classes.values() {
                let fqdn = &class_info.fqdn;

                if pattern.matches(fqdn) || pattern.matches(&class_info.simple_name) {
                    results.push(QueryResult {
                        file_path: file_path.display().to_string(),
                        line_number: class_info.position.line,
                        column: class_info.position.column,
                        symbol: class_info.simple_name.clone(),
                        fqdn: Some(fqdn.clone()),
                    });
                }
            }
        }

        Ok(results)
    }

    /// Query for field declarations
    fn query_fields(&self, pattern: &Pattern) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();

        for (file_path, file_info) in &self.type_resolver.file_infos {
            for class_info in file_info.classes.values() {
                for field in &class_info.fields {
                    let fqdn = format!("{}.{}", class_info.fqdn, field.name);

                    if pattern.matches(&fqdn) || pattern.matches(&field.name) {
                        results.push(QueryResult {
                            file_path: file_path.display().to_string(),
                            line_number: field.position.line,
                            column: field.position.column,
                            symbol: field.name.clone(),
                            fqdn: Some(fqdn),
                        });
                    }
                }
            }
        }

        Ok(results)
    }

    /// Query for method declarations
    fn query_methods(&self, pattern: &Pattern) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();

        for (file_path, file_info) in &self.type_resolver.file_infos {
            for class_info in file_info.classes.values() {
                for method in &class_info.methods {
                    let fqdn = format!("{}.{}", class_info.fqdn, method.name);

                    if pattern.matches(&fqdn) || pattern.matches(&method.name) {
                        results.push(QueryResult {
                            file_path: file_path.display().to_string(),
                            line_number: method.position.line,
                            column: method.position.column,
                            symbol: method.name.clone(),
                            fqdn: Some(fqdn),
                        });
                    }
                }
            }
        }

        Ok(results)
    }

    /// Query for enum declarations
    fn query_enums(&self, pattern: &Pattern) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();

        for (file_path, file_info) in &self.type_resolver.file_infos {
            for class_info in file_info.classes.values() {
                if class_info.is_enum {
                    let fqdn = &class_info.fqdn;

                    if pattern.matches(fqdn) || pattern.matches(&class_info.simple_name) {
                        results.push(QueryResult {
                            file_path: file_path.display().to_string(),
                            line_number: class_info.position.line,
                            column: class_info.position.column,
                            symbol: class_info.simple_name.clone(),
                            fqdn: Some(fqdn.clone()),
                        });
                    }
                }
            }
        }

        Ok(results)
    }

    /// Query for inheritance (extends clauses)
    fn query_inheritance(&self, pattern: &Pattern) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();

        for (file_path, file_info) in &self.type_resolver.file_infos {
            for class_info in file_info.classes.values() {
                if let Some(parent_simple) = &class_info.extends {
                    // Resolve to FQDN if possible
                    let parent_fqdn = self.type_resolver
                        .resolve_type_name(parent_simple, file_path)
                        .unwrap_or_else(|| parent_simple.clone());

                    if pattern.matches(&parent_fqdn) || pattern.matches(parent_simple) {
                        results.push(QueryResult {
                            file_path: file_path.display().to_string(),
                            line_number: class_info.position.line,
                            column: class_info.position.column,
                            symbol: format!("{} extends {}", class_info.simple_name, parent_simple),
                            fqdn: Some(parent_fqdn),
                        });
                    }
                }
            }
        }

        Ok(results)
    }

    /// Query for implements clauses
    fn query_implements(&self, pattern: &Pattern) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();

        for (file_path, file_info) in &self.type_resolver.file_infos {
            for class_info in file_info.classes.values() {
                for interface_simple in &class_info.implements {
                    let interface_fqdn = self.type_resolver
                        .resolve_type_name(interface_simple, file_path)
                        .unwrap_or_else(|| interface_simple.clone());

                    if pattern.matches(&interface_fqdn) || pattern.matches(interface_simple) {
                        results.push(QueryResult {
                            file_path: file_path.display().to_string(),
                            line_number: class_info.position.line,
                            column: class_info.position.column,
                            symbol: format!("{} implements {}", class_info.simple_name, interface_simple),
                            fqdn: Some(interface_fqdn),
                        });
                    }
                }
            }
        }

        Ok(results)
    }

    /// Query for method calls
    fn query_method_calls(&self, pattern: &Pattern) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();

        for (file_path, file_info) in &self.type_resolver.file_infos {
            for method_call in &file_info.method_calls {
                let method_name = &method_call.method_name;

                // Try to resolve receiver type if available
                let resolved_receiver = if let Some(receiver) = &method_call.receiver_type {
                    self.type_resolver
                        .resolve_type_name(receiver, file_path)
                        .or_else(|| Some(receiver.clone()))
                } else {
                    None
                };

                // Match against method name or receiver type
                let matches = pattern.matches(method_name)
                    || resolved_receiver.as_ref().map(|r| pattern.matches(r)).unwrap_or(false);

                if matches {
                    // Build a descriptive symbol
                    let symbol = if let Some(receiver) = &resolved_receiver {
                        format!("{}.{}", receiver, method_name)
                    } else {
                        method_name.clone()
                    };

                    results.push(QueryResult {
                        file_path: file_path.display().to_string(),
                        line_number: method_call.position.line,
                        column: method_call.position.column,
                        symbol,
                        fqdn: resolved_receiver,
                    });
                }
            }
        }

        Ok(results)
    }

    /// Query for constructor calls
    fn query_constructor_calls(&self, pattern: &Pattern) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();

        for (file_path, file_info) in &self.type_resolver.file_infos {
            for constructor_call in &file_info.constructor_calls {
                let type_name = &constructor_call.type_name;

                // Try to resolve type to FQDN
                let resolved_type = self.type_resolver
                    .resolve_type_name(type_name, file_path)
                    .unwrap_or_else(|| type_name.clone());

                // Match against type name (simple or FQDN)
                if pattern.matches(&resolved_type) || pattern.matches(type_name) {
                    results.push(QueryResult {
                        file_path: file_path.display().to_string(),
                        line_number: constructor_call.position.line,
                        column: constructor_call.position.column,
                        symbol: format!("new {}", type_name),
                        fqdn: Some(resolved_type),
                    });
                }
            }
        }

        Ok(results)
    }

    /// Query for annotations
    fn query_annotations(&self, pattern: &Pattern) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();

        for (file_path, file_info) in &self.type_resolver.file_infos {
            for annotation in &file_info.annotations {
                let annotation_name = &annotation.annotation_name;

                // Try to resolve annotation type to FQDN
                let resolved_type = self.type_resolver
                    .resolve_type_name(annotation_name, file_path)
                    .unwrap_or_else(|| annotation_name.clone());

                // Match against annotation name (simple or FQDN)
                if pattern.matches(&resolved_type) || pattern.matches(annotation_name) {
                    // Build descriptive symbol based on target
                    let symbol = match &annotation.target {
                        super::type_resolver::AnnotationTarget::Class(class) => {
                            format!("@{} on class {}", annotation_name, class)
                        }
                        super::type_resolver::AnnotationTarget::Method(class, method) => {
                            format!("@{} on {}.{}", annotation_name, class, method)
                        }
                        super::type_resolver::AnnotationTarget::Field(class, field) => {
                            format!("@{} on {}.{}", annotation_name, class, field)
                        }
                        super::type_resolver::AnnotationTarget::Parameter(class, method, param) => {
                            format!("@{} on {}.{}({})", annotation_name, class, method, param)
                        }
                        super::type_resolver::AnnotationTarget::Unknown => {
                            format!("@{}", annotation_name)
                        }
                    };

                    results.push(QueryResult {
                        file_path: file_path.display().to_string(),
                        line_number: annotation.position.line,
                        column: annotation.position.column,
                        symbol,
                        fqdn: Some(resolved_type),
                    });
                }
            }
        }

        Ok(results)
    }

    /// Query for variable declarations
    fn query_variables(&self, pattern: &Pattern) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();

        for (file_path, file_info) in &self.type_resolver.file_infos {
            for variable in &file_info.variables {
                let type_name = &variable.type_name;

                // Try to resolve type to FQDN
                let resolved_type = self.type_resolver
                    .resolve_type_name(type_name, file_path)
                    .unwrap_or_else(|| type_name.clone());

                // Match against variable type (simple or FQDN) OR variable name
                if pattern.matches(&resolved_type) ||
                   pattern.matches(type_name) ||
                   pattern.matches(&variable.variable_name) {

                    // Build descriptive symbol with context
                    let symbol = if let (Some(class_name), Some(method_name)) =
                        (&variable.class_context, &variable.method_context) {
                        format!("{} {} in {}.{}", type_name, variable.variable_name, class_name, method_name)
                    } else if let Some(method_name) = &variable.method_context {
                        format!("{} {} in {}", type_name, variable.variable_name, method_name)
                    } else {
                        format!("{} {}", type_name, variable.variable_name)
                    };

                    results.push(QueryResult {
                        file_path: file_path.display().to_string(),
                        line_number: variable.position.line,
                        column: variable.position.column,
                        symbol,
                        fqdn: Some(resolved_type),
                    });
                }
            }
        }

        Ok(results)
    }

    /// Query for return types
    fn query_return_types(&self, pattern: &Pattern) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();

        for (file_path, file_info) in &self.type_resolver.file_infos {
            for class_info in file_info.classes.values() {
                for method in &class_info.methods {
                    let return_type = &method.return_type;

                    // Try to resolve the return type to FQDN
                    let resolved_type = self.type_resolver
                        .resolve_type_name(return_type, file_path)
                        .unwrap_or_else(|| return_type.clone());

                    if pattern.matches(&resolved_type) || pattern.matches(return_type) {
                        results.push(QueryResult {
                            file_path: file_path.display().to_string(),
                            line_number: method.position.line,
                            column: method.position.column,
                            symbol: format!("{}.{}", class_info.simple_name, method.name),
                            fqdn: Some(resolved_type),
                        });
                    }
                }
            }
        }

        Ok(results)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_literal_pattern() {
        let pattern = Pattern::from_string("String").unwrap();
        assert!(pattern.matches("String"));
        assert!(!pattern.matches("string"));
        assert!(!pattern.matches("StringBuilder"));
    }

    #[test]
    fn test_wildcard_pattern() {
        let pattern = Pattern::from_string("java.util.*").unwrap();
        assert!(pattern.matches("java.util.List"));
        assert!(pattern.matches("java.util.ArrayList"));
        assert!(!pattern.matches("java.lang.String"));
    }

    #[test]
    fn test_regex_pattern() {
        let pattern = Pattern::from_string("^java\\.util\\.(List|Set)$").unwrap();
        assert!(pattern.matches("java.util.List"));
        assert!(pattern.matches("java.util.Set"));
        assert!(!pattern.matches("java.util.Map"));
        assert!(!pattern.matches("java.util.ArrayList"));
    }

    #[test]
    fn test_case_insensitive_literal() {
        let pattern = Pattern::from_string_case_insensitive("String").unwrap();
        assert!(pattern.matches("String"));
        assert!(pattern.matches("string"));
        assert!(pattern.matches("STRING"));
        assert!(!pattern.matches("StringBuilder"));
    }

    #[test]
    fn test_case_insensitive_wildcard() {
        let options = PatternOptions {
            case_insensitive: true,
            whole_word: false,
        };
        let pattern = Pattern::from_string_with_options("java.util.*", options).unwrap();
        assert!(pattern.matches("java.util.List"));
        assert!(pattern.matches("JAVA.UTIL.List"));
        assert!(pattern.matches("Java.Util.ArrayList"));
    }

    #[test]
    fn test_composite_and_pattern() {
        let pattern1 = Pattern::from_string("*Service").unwrap();
        let pattern2 = Pattern::from_string("com.example.*").unwrap();

        let composite = Pattern::and(vec![pattern1, pattern2]);

        assert!(composite.matches("com.example.UserService"));
        assert!(!composite.matches("com.other.UserService")); // Doesn't match pattern2
        assert!(!composite.matches("com.example.UserController")); // Doesn't match pattern1
    }

    #[test]
    fn test_composite_or_pattern() {
        let pattern1 = Pattern::from_string("*Controller").unwrap();
        let pattern2 = Pattern::from_string("*Service").unwrap();

        let composite = Pattern::or(vec![pattern1, pattern2]);

        assert!(composite.matches("UserController"));
        assert!(composite.matches("UserService"));
        assert!(!composite.matches("UserRepository"));
    }

    #[test]
    fn test_composite_not_pattern() {
        let pattern = Pattern::from_string("*Test").unwrap();
        let composite = Pattern::not(pattern);

        assert!(composite.matches("UserService"));
        assert!(composite.matches("UserController"));
        assert!(!composite.matches("UserTest"));
        assert!(!composite.matches("ServiceTest"));
    }

    #[test]
    fn test_complex_composite_pattern() {
        // Match: (ends with Controller OR ends with Service) AND (starts with com.example)
        let pattern1 = Pattern::or(vec![
            Pattern::from_string("*Controller").unwrap(),
            Pattern::from_string("*Service").unwrap(),
        ]);
        let pattern2 = Pattern::from_string("com.example.*").unwrap();

        let composite = Pattern::and(vec![pattern1, pattern2]);

        assert!(composite.matches("com.example.UserController"));
        assert!(composite.matches("com.example.AuthService"));
        assert!(!composite.matches("com.example.UserRepository"));
        assert!(!composite.matches("com.other.UserController"));
    }

    #[test]
    fn test_pattern_cache() {
        let cache = PatternCache::new();

        assert_eq!(cache.size(), 0);

        let pattern1 = cache.get_or_compile("java.util.*").unwrap();
        assert_eq!(cache.size(), 1);

        let pattern2 = cache.get_or_compile("java.util.*").unwrap();
        assert_eq!(cache.size(), 1); // Should reuse cached pattern

        let pattern3 = cache.get_or_compile("com.example.*").unwrap();
        assert_eq!(cache.size(), 2);

        assert!(pattern1.matches("java.util.List"));
        assert!(pattern2.matches("java.util.List"));
        assert!(pattern3.matches("com.example.Test"));

        cache.clear();
        assert_eq!(cache.size(), 0);
    }

    #[test]
    fn test_pattern_options_default() {
        let options = PatternOptions::default();
        assert!(!options.case_insensitive);
        assert!(!options.whole_word);
    }

    #[test]
    fn test_query_filters_default() {
        let filters = QueryFilters::default();
        assert!(filters.annotated.is_none());
        assert!(filters.access_modifier.is_none());
        assert!(filters.is_static.is_none());
        assert!(!filters.exclude_tests);
        assert!(!filters.deprecated_only);
    }

    #[test]
    fn test_composite_pattern_and() {
        let composite = CompositePattern::And(vec![
            Pattern::from_string("*Service").unwrap(),
            Pattern::from_string("User*").unwrap(),
        ]);

        assert!(composite.matches("UserService"));
        assert!(!composite.matches("UserController"));
        assert!(!composite.matches("AuthService"));
    }

    #[test]
    fn test_composite_pattern_or() {
        let composite = CompositePattern::Or(vec![
            Pattern::from_string("*Test").unwrap(),
            Pattern::from_string("*TestCase").unwrap(),
        ]);

        assert!(composite.matches("UserTest"));
        assert!(composite.matches("AuthTestCase"));
        assert!(!composite.matches("UserService"));
    }

    #[test]
    fn test_composite_pattern_not() {
        let composite = CompositePattern::Not(Box::new(
            Pattern::from_string("*Test").unwrap()
        ));

        assert!(composite.matches("UserService"));
        assert!(!composite.matches("UserTest"));
    }
}
