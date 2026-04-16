# Phase 2: Full Location Type Implementation - Plan

**Status**: Planning  
**Date**: April 14, 2026

---

## Overview

Phase 2 completes the Java analyzer by implementing the remaining location types, adding source location extraction, and integrating with the Konveyor provider interface.

## Goals

1. Complete all 15 location types with accurate source locations
2. Implement provider gRPC interface
3. Add dependency resolution (Maven/Gradle)
4. Optimize performance for large codebases

---

## Task Breakdown

### Task 2.1: Source Location Extraction ⭐ START HERE

**Priority**: High - Needed for all queries  
**Effort**: Medium (~2-3 hours)

**Goal**: Extract accurate line numbers and columns from AST nodes for all query results.

**Approach**:
1. Store AST node positions during TypeResolver analysis
2. Add position tracking to ClassInfo, MethodInfo, FieldInfo
3. Update query methods to return real positions instead of 0

**Implementation**:
```rust
// Add to ClassInfo, MethodInfo, FieldInfo
pub struct ClassInfo {
    // ... existing fields ...
    pub position: SourcePosition,
}

pub struct SourcePosition {
    pub line: usize,        // 1-based
    pub column: usize,      // 0-based
    pub end_line: usize,
    pub end_column: usize,
}
```

**Files to Modify**:
- `src/java_graph/type_resolver.rs` - Add position tracking during extraction
- `src/java_graph/query.rs` - Use positions in QueryResult
- Tests - Verify positions are correct

**Success Criteria**:
- [ ] All ClassInfo has accurate source positions
- [ ] All MethodInfo has accurate source positions
- [ ] All FieldInfo has accurate source positions
- [ ] QueryResult returns real line/column numbers
- [ ] Tests verify positions against known fixture locations

---

### Task 2.2: Method Call Tracking

**Priority**: High - Core location type  
**Effort**: Medium-High (~3-4 hours)

**Goal**: Implement `query_method_calls()` to find method invocations.

**Approach**:
1. Store method invocations during file analysis
2. Resolve method call types using TypeResolver
3. Match against query pattern

**Implementation**:
```rust
pub struct MethodCall {
    pub method_name: String,
    pub receiver_type: Option<String>,  // Type of object being called
    pub position: SourcePosition,
}

// Add to FileInfo
pub struct FileInfo {
    // ... existing fields ...
    pub method_calls: Vec<MethodCall>,
}
```

**AST Pattern**:
```
method_invocation
  ├─ object (optional)
  ├─ name: (identifier)
  └─ arguments
```

**Files to Modify**:
- `src/java_graph/type_resolver.rs` - Extract method calls during analysis
- `src/java_graph/query.rs` - Implement `query_method_calls()`
- Tests - Verify method call detection

**Success Criteria**:
- [ ] Extract all method_invocation nodes
- [ ] Resolve receiver types when possible
- [ ] Match against simple name or FQDN
- [ ] Handle chained calls (e.g., `obj.method1().method2()`)
- [ ] Tests cover various invocation patterns

---

### Task 2.3: Constructor Call Tracking

**Priority**: High - Core location type  
**Effort**: Medium (~2-3 hours)

**Goal**: Implement `query_constructor_calls()` to find `new` expressions.

**Approach**:
1. Store object creation expressions during analysis
2. Resolve constructor types
3. Match against pattern

**Implementation**:
```rust
pub struct ConstructorCall {
    pub type_name: String,
    pub resolved_type: Option<String>,  // FQDN
    pub position: SourcePosition,
}

// Add to FileInfo
pub method FileInfo {
    // ... existing fields ...
    pub constructor_calls: Vec<ConstructorCall>,
}
```

**AST Pattern**:
```
object_creation_expression
  ├─ type: (type_identifier | generic_type)
  └─ arguments
```

**Files to Modify**:
- `src/java_graph/type_resolver.rs` - Extract constructor calls
- `src/java_graph/query.rs` - Implement `query_constructor_calls()`
- Tests - Verify constructor detection

**Success Criteria**:
- [ ] Extract all object_creation_expression nodes
- [ ] Resolve type names to FQDNs
- [ ] Handle generic types (e.g., `new ArrayList<String>()`)
- [ ] Match against type pattern
- [ ] Tests cover various constructor patterns

---

### Task 2.4: Annotation Tracking

**Priority**: Medium - Common in modern Java  
**Effort**: Medium (~2-3 hours)

**Goal**: Implement `query_annotations()` to find annotation usage.

**Approach**:
1. Store annotations during analysis
2. Track what they're attached to (class, method, field, parameter)
3. Match against annotation type pattern

**Implementation**:
```rust
pub struct AnnotationUsage {
    pub annotation_name: String,
    pub resolved_name: Option<String>,  // FQDN
    pub target: AnnotationTarget,
    pub position: SourcePosition,
}

pub enum AnnotationTarget {
    Class(String),
    Method(String),
    Field(String),
    Parameter(String, String),  // (method, param)
}

// Add to FileInfo
pub struct FileInfo {
    // ... existing fields ...
    pub annotations: Vec<AnnotationUsage>,
}
```

**AST Patterns**:
```
marker_annotation
  └─ name: (identifier | scoped_identifier)

annotation
  ├─ name: (identifier | scoped_identifier)
  └─ arguments
```

**Files to Modify**:
- `src/java_graph/type_resolver.rs` - Extract annotations
- `src/java_graph/query.rs` - Implement `query_annotations()`
- Tests - Verify annotation detection

**Success Criteria**:
- [ ] Extract marker annotations (@Override, @Deprecated)
- [ ] Extract annotations with parameters (@SuppressWarnings("unused"))
- [ ] Resolve annotation types to FQDNs
- [ ] Track annotation targets
- [ ] Match against annotation type pattern
- [ ] Tests cover class, method, field annotations

---

### Task 2.5: Variable Tracking

**Priority**: Medium - Less common in queries  
**Effort**: Medium (~2-3 hours)

**Goal**: Implement `query_variables()` to find local variable declarations.

**Approach**:
1. Store local variable declarations during analysis
2. Track variable type and scope
3. Match against type pattern

**Implementation**:
```rust
pub struct LocalVariable {
    pub name: String,
    pub type_name: String,
    pub resolved_type: Option<String>,  // FQDN
    pub method: String,  // Which method contains this variable
    pub position: SourcePosition,
}

// Add to FileInfo
pub struct FileInfo {
    // ... existing fields ...
    pub local_variables: Vec<LocalVariable>,
}
```

**AST Pattern**:
```
local_variable_declaration
  ├─ type
  └─ variable_declarator
      └─ name: (identifier)
```

**Files to Modify**:
- `src/java_graph/type_resolver.rs` - Extract local variables
- `src/java_graph/query.rs` - Implement `query_variables()`
- Tests - Verify variable detection

**Success Criteria**:
- [ ] Extract local variable declarations
- [ ] Resolve variable types to FQDNs
- [ ] Track containing method/class
- [ ] Match against type pattern
- [ ] Tests cover various variable patterns

---

### Task 2.6: Provider gRPC Interface

**Priority**: High - Required for Konveyor integration  
**Effort**: High (~4-6 hours)

**Goal**: Implement the Konveyor provider gRPC interface.

**Approach**:
1. Define protobuf schema for Java analyzer
2. Implement gRPC server
3. Connect QueryEngine to provider methods
4. Handle initialization, capabilities, evaluation

**Protobuf Schema** (already exists in konveyor/analyzer-lsp):
```protobuf
service ProviderService {
    rpc Init(InitRequest) returns (InitResponse);
    rpc GetCapabilities(Empty) returns (Capabilities);
    rpc Evaluate(EvaluateRequest) returns (EvaluateResponse);
}
```

**Implementation**:
```rust
pub struct JavaProvider {
    query_engine: QueryEngine,
    base_path: PathBuf,
}

impl ProviderService for JavaProvider {
    fn init(&self, request: InitRequest) -> Result<InitResponse>;
    fn get_capabilities(&self) -> Result<Capabilities>;
    fn evaluate(&self, request: EvaluateRequest) -> Result<EvaluateResponse>;
}
```

**Files to Create/Modify**:
- `src/provider/java_provider.rs` - Main provider implementation
- `src/provider/mod.rs` - Provider module
- `build.rs` - Protobuf compilation
- `proto/provider.proto` - Schema (or import from konveyor)
- Tests - Provider integration tests

**Success Criteria**:
- [ ] gRPC server starts and accepts connections
- [ ] Init initializes TypeResolver and QueryEngine
- [ ] GetCapabilities returns supported location types
- [ ] Evaluate handles query requests and returns results
- [ ] Results match Konveyor's expected format
- [ ] Integration test with mock Konveyor client

---

### Task 2.7: Dependency Resolution (Maven)

**Priority**: Medium - Needed for external type resolution  
**Effort**: High (~5-6 hours)

**Goal**: Resolve Maven dependencies and add to type index.

**Approach**:
1. Parse pom.xml to extract dependencies
2. Download JARs from Maven Central
3. Parse JAR metadata (class signatures)
4. Add external types to global index

**Implementation**:
```rust
pub struct MavenResolver {
    local_repo: PathBuf,  // ~/.m2/repository
    cache: HashMap<String, Vec<String>>,  // Artifact -> Classes
}

impl MavenResolver {
    pub fn resolve_dependencies(&self, pom_path: &Path) -> Result<Vec<Dependency>>;
    pub fn download_jar(&self, dependency: &Dependency) -> Result<PathBuf>;
    pub fn parse_jar_classes(&self, jar_path: &Path) -> Result<Vec<String>>;
}
```

**Files to Create**:
- `src/dependency/maven.rs` - Maven resolution
- `src/dependency/jar_parser.rs` - JAR class extraction
- `src/dependency/mod.rs` - Dependency module
- Tests - Maven resolution tests

**Success Criteria**:
- [ ] Parse pom.xml correctly
- [ ] Resolve transitive dependencies
- [ ] Download JARs from Maven Central
- [ ] Extract class names from JARs
- [ ] Add external types to TypeResolver global index
- [ ] Queries can match against external types

---

### Task 2.8: Dependency Resolution (Gradle)

**Priority**: Medium - Alternative to Maven  
**Effort**: Medium-High (~4-5 hours)

**Goal**: Resolve Gradle dependencies.

**Approach**:
1. Parse build.gradle or build.gradle.kts
2. Execute Gradle to get dependency tree
3. Process JARs similar to Maven

**Implementation**:
```rust
pub struct GradleResolver {
    gradle_home: PathBuf,
    cache: HashMap<String, Vec<String>>,
}

impl GradleResolver {
    pub fn resolve_dependencies(&self, build_file: &Path) -> Result<Vec<Dependency>>;
}
```

**Files to Create**:
- `src/dependency/gradle.rs` - Gradle resolution
- Tests - Gradle resolution tests

**Success Criteria**:
- [ ] Parse build.gradle (Groovy)
- [ ] Parse build.gradle.kts (Kotlin)
- [ ] Execute `gradle dependencies` to get resolved tree
- [ ] Process JARs to extract classes
- [ ] Integration with TypeResolver

---

### Task 2.9: Performance Optimization

**Priority**: Low - Optimize after functionality complete  
**Effort**: Medium (~3-4 hours)

**Goal**: Optimize for large codebases (1000+ files).

**Approaches**:
1. **Parallel Analysis**: Use rayon to analyze files concurrently
2. **Incremental Updates**: Only re-analyze changed files
3. **Query Caching**: Cache query results
4. **Index Optimization**: Use better data structures for lookups

**Implementation**:
```rust
use rayon::prelude::*;

impl TypeResolver {
    pub fn analyze_files_parallel(&mut self, paths: &[PathBuf]) -> Result<()> {
        let results: Vec<_> = paths.par_iter()
            .map(|path| self.analyze_file(path))
            .collect();
        
        // Merge results
        for result in results {
            // ...
        }
        Ok(())
    }
}
```

**Files to Modify**:
- `src/java_graph/type_resolver.rs` - Add parallel analysis
- `src/java_graph/query.rs` - Add query caching
- Benchmarks - Performance tests

**Success Criteria**:
- [ ] Analyze 1000 files in < 10 seconds
- [ ] Query 1000 files in < 100ms
- [ ] Memory usage scales linearly
- [ ] Incremental updates work correctly

---

### Task 2.10: Enhanced Pattern Matching

**Priority**: Low - Nice to have  
**Effort**: Medium (~2-3 hours)

**Goal**: Support advanced pattern matching features.

**Features**:
1. **Method Signatures**: `"MyClass.myMethod(String, int)"`
2. **Annotation Parameters**: `"@SuppressWarnings(value=\"unused\")"`
3. **Generic Types**: `"List<String>"`

**Implementation**:
```rust
pub enum Pattern {
    Literal(String),
    Wildcard(String),
    Regex(Regex),
    Signature(MethodSignature),  // NEW
    AnnotationWithParams(String, HashMap<String, String>),  // NEW
}

pub struct MethodSignature {
    pub class_pattern: Option<String>,
    pub method_name: String,
    pub parameters: Vec<String>,
}
```

**Files to Modify**:
- `src/java_graph/query.rs` - Enhanced pattern parsing
- Tests - Pattern matching tests

**Success Criteria**:
- [ ] Parse method signatures
- [ ] Match methods by signature
- [ ] Match annotations by parameters
- [ ] Tests cover all pattern types

---

## Implementation Order

### Phase 2A: Complete Location Types (Weeks 1-2)
1. **Task 2.1**: Source Location Extraction ⭐
2. **Task 2.2**: Method Call Tracking
3. **Task 2.3**: Constructor Call Tracking
4. **Task 2.4**: Annotation Tracking
5. **Task 2.5**: Variable Tracking

**Goal**: All 15 location types working with accurate positions.

### Phase 2B: Provider Integration (Week 3)
6. **Task 2.6**: Provider gRPC Interface

**Goal**: Konveyor can query the analyzer via gRPC.

### Phase 2C: Dependency Resolution (Week 4)
7. **Task 2.7**: Maven Dependency Resolution
8. **Task 2.8**: Gradle Dependency Resolution

**Goal**: External types resolved and queryable.

### Phase 2D: Optimization (Week 5)
9. **Task 2.9**: Performance Optimization
10. **Task 2.10**: Enhanced Pattern Matching

**Goal**: Production-ready performance and features.

---

## Testing Strategy

Each task must have:
- **Unit Tests**: Test individual functions
- **Integration Tests**: Test end-to-end workflows
- **Fixture Tests**: Test against real Java code samples

**Test Coverage Goal**: > 90% for all Phase 2 code

---

## Success Metrics

Phase 2 is complete when:
- ✅ All 15 location types return accurate results with source positions
- ✅ Provider gRPC interface works with Konveyor
- ✅ Maven and Gradle dependencies resolve correctly
- ✅ Performance targets met (1000 files < 10s)
- ✅ All tests pass (target: 150+ total tests)
- ✅ Documentation complete for all features

---

## Next Step

**START HERE**: Task 2.1 - Source Location Extraction

This is the foundation for all other tasks. Once positions are tracked, we can incrementally add the remaining location types.

Ready to begin? Let's implement Task 2.1!
