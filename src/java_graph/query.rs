// Query engine for location types

use anyhow::{Context, Result};
use regex::Regex;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tracing::debug;

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
    /// Get the pattern string (for Literal and Wildcard patterns)
    pub fn as_string(&self) -> Option<&str> {
        match self {
            Pattern::Literal(s, _) => Some(s),
            Pattern::Wildcard(s, _) => Some(s),
            Pattern::Regex(_, _) => None,
            Pattern::Composite(_) => None,
        }
    }

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

/// Annotation filter for querying annotated elements
#[derive(Debug, Clone)]
pub struct AnnotationFilter {
    pub pattern: Option<String>,  // Annotation pattern (e.g., "javax.inject.Inject")
    pub elements: HashMap<String, String>,  // Required elements (name -> value)
}

/// Query filters for advanced filtering
#[derive(Debug, Clone, Default)]
pub struct QueryFilters {
    /// Optional annotation filter (match elements with this annotation)
    pub annotated: Option<AnnotationFilter>,
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
    pub annotated: Option<AnnotationFilter>,  // Optional annotation filter
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
    type_resolver: TypeResolver,
    pattern_cache: PatternCache,
}

impl QueryEngine {
    /// Create a new query engine
    pub fn new(type_resolver: TypeResolver) -> Self {
        QueryEngine {
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
            LocationType::Type => self.query_types(&query.pattern, query.annotated.as_ref()),
            LocationType::Field => self.query_fields(&query.pattern, query.annotated.as_ref()),
            LocationType::Method => self.query_methods(&query.pattern, query.annotated.as_ref()),
            LocationType::Enum => self.query_enums(&query.pattern),
            LocationType::Inheritance => self.query_inheritance(&query.pattern),
            LocationType::ImplementsType => self.query_implements(&query.pattern),
            LocationType::MethodCall => self.query_method_calls(&query.pattern),
            LocationType::ConstructorCall => self.query_constructor_calls(&query.pattern),
            LocationType::Annotation => self.query_annotations(&query.pattern, query.annotated.as_ref()),
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

    /// Check if annotations match the annotation filter
    fn matches_annotation_filter(
        annotations: &[super::type_resolver::AnnotationInfo],
        filter: &AnnotationFilter,
    ) -> bool {
        // If no annotations on the element, filter doesn't match
        if annotations.is_empty() {
            return false;
        }

        // Check each annotation against the filter
        for annotation in annotations {
            // Match pattern if specified
            if let Some(ref pattern_str) = filter.pattern {
                let pattern = match Pattern::from_string(pattern_str) {
                    Ok(p) => p,
                    Err(_) => continue,  // Invalid pattern, skip this annotation
                };

                // Check if annotation name or FQDN matches the pattern
                let matches_pattern = if let Some(ref fqdn) = annotation.fqdn {
                    pattern.matches(fqdn) || pattern.matches(&annotation.name)
                } else {
                    pattern.matches(&annotation.name)
                };

                if !matches_pattern {
                    continue;  // This annotation doesn't match the pattern
                }
            }

            // If pattern matches (or no pattern specified), check elements
            if !filter.elements.is_empty() {
                // All required elements must match
                let mut all_elements_match = true;
                for (required_name, required_value) in &filter.elements {
                    if let Some(actual_value) = annotation.elements.get(required_name) {
                        // Try to match as a regex pattern first
                        let value_matches = if let Ok(value_pattern) = Pattern::from_string(required_value) {
                            value_pattern.matches(actual_value)
                        } else {
                            // If pattern creation fails, do exact match
                            actual_value == required_value
                        };

                        if !value_matches {
                            all_elements_match = false;
                            break;
                        }
                    } else {
                        // Required element not present
                        all_elements_match = false;
                        break;
                    }
                }

                if all_elements_match {
                    return true;  // This annotation matches completely
                }
            } else {
                // No elements required, pattern match is enough
                return true;
            }
        }

        // No annotation matched the filter
        false
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

    /// Query for package declarations and import package references
    fn query_packages(&self, pattern: &Pattern) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();
        let mut seen_packages = std::collections::HashSet::new();

        for (file_path, file_info) in &self.type_resolver.file_infos {
            // Match against package declaration
            if let Some(package_name) = &file_info.package_name {
                if pattern.matches(package_name) && seen_packages.insert((file_path.clone(), package_name.clone())) {
                    results.push(QueryResult {
                        file_path: file_path.display().to_string(),
                        line_number: 0,  // TODO: Extract from AST
                        column: 0,
                        symbol: package_name.clone(),
                        fqdn: Some(package_name.clone()),
                    });
                }
            }

            // Match against import packages (extract package from FQDN)
            for (_simple_name, fqdn) in &file_info.explicit_imports {
                // Extract package part (everything before the last '.')
                if let Some(last_dot) = fqdn.rfind('.') {
                    let package_part = &fqdn[..last_dot];
                    if pattern.matches(package_part) && seen_packages.insert((file_path.clone(), package_part.to_string())) {
                        results.push(QueryResult {
                            file_path: file_path.display().to_string(),
                            line_number: 0,  // TODO: Extract from AST
                            column: 0,
                            symbol: package_part.to_string(),
                            fqdn: Some(package_part.to_string()),
                        });
                    }
                }
            }

            // Match against wildcard imports (they are already packages)
            for wildcard_pkg in &file_info.wildcard_imports {
                if pattern.matches(wildcard_pkg) && seen_packages.insert((file_path.clone(), wildcard_pkg.clone())) {
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
    fn query_types(&self, pattern: &Pattern, annotation_filter: Option<&AnnotationFilter>) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();

        for (file_path, file_info) in &self.type_resolver.file_infos {
            for class_info in file_info.classes.values() {
                // Check annotation filter first
                if let Some(filter) = annotation_filter {
                    if !Self::matches_annotation_filter(&class_info.annotations, filter) {
                        continue;  // Class doesn't have required annotation
                    }
                }

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
    fn query_fields(&self, pattern: &Pattern, annotation_filter: Option<&AnnotationFilter>) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();

        for (file_path, file_info) in &self.type_resolver.file_infos {
            for class_info in file_info.classes.values() {
                for field in &class_info.fields {
                    // Check annotation filter first
                    if let Some(filter) = annotation_filter {
                        if !Self::matches_annotation_filter(&field.annotations, filter) {
                            continue;  // Field doesn't have required annotation
                        }
                    }

                    // Resolve field type to FQDN
                    let field_type_fqdn = self.type_resolver
                        .resolve_type_name(&field.type_name, file_path)
                        .unwrap_or_else(|| field.type_name.clone());

                    debug!(
                        "Field query - File: {}, Class: {}, Field: {}, Type: {} -> FQDN: {}",
                        file_path.display(),
                        class_info.simple_name,
                        field.name,
                        field.type_name,
                        field_type_fqdn
                    );

                    // For FIELD location, pattern matches against the field TYPE, not field name
                    // Pattern can be:
                    // 1. Just type: "CustomerRepository" matches any field of that type
                    // 2. fieldName + type: "repository CustomerRepository" or "* CustomerRepository"

                    // Check if pattern contains a space (field name + type pattern)
                    let matches = if let Some(pattern_str) = pattern.as_string() {
                        if pattern_str.contains(' ') {
                            // Pattern like "* TypedEntity" or "repository CustomerRepository"
                            let parts: Vec<&str> = pattern_str.splitn(2, ' ').collect();
                            if parts.len() == 2 {
                                let field_name_pattern = parts[0];
                                let field_type_pattern = parts[1];

                                // Match field name (support * wildcard)
                                let name_matches = field_name_pattern == "*" ||
                                                   field_name_pattern == &field.name ||
                                                   Pattern::from_string(field_name_pattern)
                                                       .ok()
                                                       .map(|p| p.matches(&field.name))
                                                       .unwrap_or(false);

                                // Match field type
                                let type_matches = Pattern::from_string(field_type_pattern)
                                    .ok()
                                    .map(|p| p.matches(&field_type_fqdn) || p.matches(&field.type_name))
                                    .unwrap_or(false);

                                name_matches && type_matches
                            } else {
                                false
                            }
                        } else {
                            // Pattern is just a type name
                            pattern.matches(&field_type_fqdn) || pattern.matches(&field.type_name)
                        }
                    } else {
                        // For regex/composite patterns, try matching against type
                        pattern.matches(&field_type_fqdn) || pattern.matches(&field.type_name)
                    };

                    if matches {
                        results.push(QueryResult {
                            file_path: file_path.display().to_string(),
                            line_number: field.position.line,
                            column: field.position.column,
                            symbol: format!("{}: {}", field.name, field.type_name),
                            fqdn: Some(field_type_fqdn),
                        });
                    }
                }
            }
        }

        Ok(results)
    }

    /// Query for method declarations
    fn query_methods(&self, pattern: &Pattern, annotation_filter: Option<&AnnotationFilter>) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();

        for (file_path, file_info) in &self.type_resolver.file_infos {
            for class_info in file_info.classes.values() {
                for method in &class_info.methods {
                    // Check annotation filter first
                    if let Some(filter) = annotation_filter {
                        let matches = Self::matches_annotation_filter(&method.annotations, filter);
                        debug!(
                            "Method {}.{} annotation filter check: {} (annotations: {:?}, filter: pattern={:?}, elements={:?})",
                            class_info.simple_name,
                            method.name,
                            matches,
                            method.annotations.iter().map(|a| (&a.name, &a.elements)).collect::<Vec<_>>(),
                            filter.pattern,
                            filter.elements
                        );
                        if !matches {
                            continue;  // Method doesn't have required annotation
                        }
                    }

                    let fqdn = format!("{}.{}", class_info.fqdn, method.name);
                    let simple_class_method = format!("{}.{}", class_info.simple_name, method.name);

                    // Resolve method return type to FQDN
                    let return_type_fqdn = self.type_resolver
                        .resolve_type_name(&method.return_type, file_path)
                        .unwrap_or_else(|| method.return_type.clone());

                    // Check if pattern contains a space (method name + return type pattern)
                    let matches = if let Some(pattern_str) = pattern.as_string() {
                        if pattern_str.contains(' ') {
                            // Pattern like "* TilesConfigurer" or "methodName ReturnType"
                            let parts: Vec<&str> = pattern_str.splitn(2, ' ').collect();
                            if parts.len() == 2 {
                                let method_name_pattern = parts[0];
                                let return_type_pattern = parts[1];

                                // Match method name (support * wildcard)
                                let name_matches = method_name_pattern == "*" ||
                                                   method_name_pattern == &method.name ||
                                                   Pattern::from_string(method_name_pattern)
                                                       .ok()
                                                       .map(|p| p.matches(&method.name))
                                                       .unwrap_or(false);

                                // Match return type
                                let type_matches = Pattern::from_string(return_type_pattern)
                                    .ok()
                                    .map(|p| p.matches(&return_type_fqdn) || p.matches(&method.return_type))
                                    .unwrap_or(false);

                                name_matches && type_matches
                            } else {
                                false
                            }
                        } else {
                            // Pattern is just a method name or class.method
                            pattern.matches(&fqdn) || pattern.matches(&simple_class_method) || pattern.matches(&method.name)
                        }
                    } else {
                        // For regex/composite patterns, try matching against method names
                        pattern.matches(&fqdn) || pattern.matches(&simple_class_method) || pattern.matches(&method.name)
                    };

                    if matches {
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
                let resolved_receiver_fqdn = if let Some(receiver_name) = &method_call.receiver_type {
                    // First, try to resolve as a type name directly (for static calls or class references)
                    if let Some(fqdn) = self.type_resolver.resolve_type_name(receiver_name, file_path) {
                        Some(fqdn)
                    } else {
                        // If not a type, it might be a field/variable name
                        // Look for a field with this name in all classes in this file
                        let mut field_type = None;
                        for class_info in file_info.classes.values() {
                            if let Some(field) = class_info.fields.iter().find(|f| &f.name == receiver_name) {
                                // Found the field, now resolve its type
                                field_type = self.type_resolver
                                    .resolve_type_name(&field.type_name, file_path)
                                    .or_else(|| Some(field.type_name.clone()));
                                break;
                            }
                        }
                        field_type
                    }
                } else {
                    None
                };

                // Build possible patterns to match:
                // 1. Full FQDN: com.example.service.HomeService.doThings
                // 2. Simple class name: HomeService.doThings
                // 3. Just method name: doThings

                let fqdn_pattern = resolved_receiver_fqdn.as_ref()
                    .map(|fqdn| format!("{}.{}", fqdn, method_name));

                let simple_class_pattern = resolved_receiver_fqdn.as_ref()
                    .and_then(|fqdn| fqdn.rfind('.'))
                    .map(|last_dot| {
                        let simple_name = &resolved_receiver_fqdn.as_ref().unwrap()[last_dot + 1..];
                        format!("{}.{}", simple_name, method_name)
                    });

                // Match against any of the patterns
                let matches = pattern.matches(method_name)
                    || fqdn_pattern.as_ref().map(|p| pattern.matches(p)).unwrap_or(false)
                    || simple_class_pattern.as_ref().map(|p| pattern.matches(p)).unwrap_or(false);

                if matches {
                    // Build a descriptive symbol
                    let symbol = fqdn_pattern.unwrap_or_else(|| method_name.clone());

                    results.push(QueryResult {
                        file_path: file_path.display().to_string(),
                        line_number: method_call.position.line,
                        column: method_call.position.column,
                        symbol,
                        fqdn: resolved_receiver_fqdn,
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
    fn query_annotations(&self, pattern: &Pattern, annotation_filter: Option<&AnnotationFilter>) -> Result<Vec<QueryResult>> {
        let mut results = Vec::new();

        // Search through class, method, and field annotations (which have AnnotationInfo with elements)
        for (file_path, file_info) in &self.type_resolver.file_infos {
            for class_info in file_info.classes.values() {
                // Check class annotations
                for annotation in &class_info.annotations {
                    if let Some(result) = self.check_annotation_match(
                        annotation,
                        pattern,
                        annotation_filter,
                        file_path,
                        &format!("@{} on class {}", annotation.name, class_info.simple_name),
                    ) {
                        results.push(result);
                    }
                }

                // Check method annotations
                for method in &class_info.methods {
                    for annotation in &method.annotations {
                        if let Some(result) = self.check_annotation_match(
                            annotation,
                            pattern,
                            annotation_filter,
                            file_path,
                            &format!("@{} on {}.{}", annotation.name, class_info.simple_name, method.name),
                        ) {
                            results.push(result);
                        }
                    }
                }

                // Check field annotations
                for field in &class_info.fields {
                    for annotation in &field.annotations {
                        if let Some(result) = self.check_annotation_match(
                            annotation,
                            pattern,
                            annotation_filter,
                            file_path,
                            &format!("@{} on {}.{}", annotation.name, class_info.simple_name, field.name),
                        ) {
                            results.push(result);
                        }
                    }
                }
            }
        }

        Ok(results)
    }

    /// Helper to check if an annotation matches pattern and filter
    fn check_annotation_match(
        &self,
        annotation: &super::type_resolver::AnnotationInfo,
        pattern: &Pattern,
        annotation_filter: Option<&AnnotationFilter>,
        file_path: &std::path::Path,
        symbol: &str,
    ) -> Option<QueryResult> {
        // Check if annotation name or FQDN matches the pattern
        let annotation_fqdn = annotation.fqdn.as_ref().unwrap_or(&annotation.name);

        // Match against FQDN, simple name, or try to resolve via wildcard imports
        let mut pattern_matches = pattern.matches(annotation_fqdn) || pattern.matches(&annotation.name);

        // If no match yet and annotation doesn't have FQDN, try wildcard import resolution
        if !pattern_matches && annotation.fqdn.is_none() {
            // Get file info to check wildcard imports
            if let Some(file_info) = self.type_resolver.file_infos.get(file_path) {
                for wildcard_pkg in &file_info.wildcard_imports {
                    let candidate_fqdn = format!("{}.{}", wildcard_pkg, annotation.name);
                    if pattern.matches(&candidate_fqdn) {
                        pattern_matches = true;
                        break;
                    }
                }
            }
        }

        if !pattern_matches {
            return None;  // Pattern doesn't match
        }

        // Check annotation filter (filter by annotation's own elements)
        if let Some(filter) = annotation_filter {
            // If filter has a pattern, it should match (already checked above, but double-check)
            if let Some(ref filter_pattern_str) = filter.pattern {
                let filter_pattern = match Pattern::from_string(filter_pattern_str) {
                    Ok(p) => p,
                    Err(_) => return None,
                };
                if !filter_pattern.matches(annotation_fqdn) && !filter_pattern.matches(&annotation.name) {
                    return None;
                }
            }

            // Check that all required elements match
            for (required_name, required_value) in &filter.elements {
                if let Some(actual_value) = annotation.elements.get(required_name) {
                    // Check if value matches (could be exact or regex)
                    if !Self::value_matches(actual_value, required_value) {
                        return None;  // Element doesn't match
                    }
                } else {
                    return None;  // Required element not present
                }
            }
        }

        // Annotation matches!
        Some(QueryResult {
            file_path: file_path.display().to_string(),
            line_number: annotation.position.line,
            column: annotation.position.column,
            symbol: symbol.to_string(),
            fqdn: Some(annotation_fqdn.clone()),
        })
    }

    /// Check if a value matches (supports regex patterns)
    fn value_matches(actual: &str, expected: &str) -> bool {
        // Try as regex first
        if let Ok(regex) = Regex::new(expected) {
            if regex.is_match(actual) {
                return true;
            }
        }

        // Fallback to exact match
        actual == expected
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

    #[test]
    fn test_annotation_element_regex_matching() {
        use super::super::type_resolver::{AnnotationInfo, SourcePosition};
        use std::collections::HashMap;

        // Simulate the @Bean annotation from TilesConfig.java
        let mut elements = HashMap::new();
        elements.insert("name".to_string(), "nameForThisBean".to_string());
        elements.insert("autowireCandidate".to_string(), "false".to_string());

        let annotation = AnnotationInfo {
            name: "Bean".to_string(),
            fqdn: Some("org.springframework.context.annotation.Bean".to_string()),
            elements,
            position: SourcePosition { line: 16, column: 5, end_line: 16, end_column: 50 },
        };

        // Create filter matching rule konveyor-java-pattern-test-21
        let mut filter_elements = HashMap::new();
        filter_elements.insert("name".to_string(), "nameFor.*".to_string());
        filter_elements.insert("autowireCandidate".to_string(), "false".to_string());

        let filter = AnnotationFilter {
            pattern: Some("org.springframework.context.annotation.Bean".to_string()),
            elements: filter_elements,
        };

        // Test the matching
        let annotations = vec![annotation];
        let result = QueryEngine::matches_annotation_filter(&annotations, &filter);

        assert!(result, "Annotation should match the filter with regex pattern 'nameFor.*' matching 'nameForThisBean' and literal 'false' matching 'false'");
    }

    #[test]
    fn test_tiles_config_annotation_extraction() {
        use super::super::type_resolver::TypeResolver;
        use std::path::PathBuf;

        let file_path = PathBuf::from("e2e-tests/examples/sample-tiles-app/src/main/java/com/example/config/TilesConfig.java");

        // Skip test if file doesn't exist (e.g., in CI without e2e tests)
        if !file_path.exists() {
            println!("Skipping test - file not found: {}", file_path.display());
            return;
        }

        let mut resolver = TypeResolver::new();
        resolver.analyze_file(&file_path).unwrap();

        // Find TilesConfig class in the resolver's file_infos
        let file_info = resolver.file_infos.get(&file_path).expect("File info should exist");
        let tiles_config = file_info.classes.get("TilesConfig").expect("TilesConfig class should exist");

        // Find tilesConfigurer method
        let tiles_configurer_method = tiles_config.methods.iter()
            .find(|m| m.name == "tilesConfigurer")
            .expect("tilesConfigurer method should exist");

        // Check method has annotations
        assert!(!tiles_configurer_method.annotations.is_empty(), "tilesConfigurer method should have annotations");

        // Find @Bean annotation
        let bean_annotation = tiles_configurer_method.annotations.iter()
            .find(|a| a.name == "Bean")
            .expect("@Bean annotation should exist");

        println!("Bean annotation elements: {:?}", bean_annotation.elements);

        // Check annotation elements
        assert!(bean_annotation.elements.contains_key("name"), "Bean annotation should have 'name' element");
        assert_eq!(bean_annotation.elements.get("name").unwrap(), "nameForThisBean", "Bean 'name' element should be 'nameForThisBean'");

        assert!(bean_annotation.elements.contains_key("autowireCandidate"), "Bean annotation should have 'autowireCandidate' element");
        assert_eq!(bean_annotation.elements.get("autowireCandidate").unwrap(), "false", "Bean 'autowireCandidate' element should be 'false'");
    }
}
