# Java Provider Implementation Plan

**Project**: Self-contained Rust-based Java analyzer provider for Konveyor  
**Design Document**: `java-provider-design.md`  
**Date**: April 13, 2026

---

## Overview

**Goal**: Build a pure Rust Java analyzer provider that eliminates the JDTLS language server dependency while maintaining 100% API compatibility with the existing Go-based Java provider.

**Architecture**: tree-sitter-java + stack-graphs + TypeResolver + Maven/Gradle integration

**Target Capabilities**:
- "referenced": 15 location types (type, inheritance, implements_type, method_call, constructor_call, annotation, return_type, import, variable_declaration, package, field, method, class, enum)
- "dependency": Full Maven/Gradle dependency tree analysis

---

## Prerequisites

### Required Tools
- Rust toolchain (2021 edition or later)
- Java runtime (for FernFlower decompilation)
- Maven (for testing Maven integration)
- Gradle (for testing Gradle integration)
- Git (for cloning C# provider reference)

### Reference Materials
- C# provider codebase: `/home/jmle/Dev/redhat/c-sharp-analyzer-provider`
- Design document: `java-provider-design.md`
- Go Java provider: https://github.com/konveyor/analyzer-lsp/tree/main/external-providers/java-external-provider

### Test Projects
- Simple Maven project for basic testing
- Spring PetClinic (complex Spring Boot app)
- Gradle multi-module project
- JAR/WAR/EAR binary artifacts

---

## Phase 1: Foundation & Basic Analysis

### Task 1.1: Project Setup

**Objective**: Initialize Rust project with core dependencies

**Steps**:
1. Create new Rust project: `cargo new java-analyzer-provider`
2. Copy protobuf definitions from C# provider (`src/build/proto/provider.proto`)
3. Set up `build.rs` for protobuf compilation (reference C# provider)
4. Add core dependencies to `Cargo.toml`:
   - tokio, tonic (gRPC)
   - tree-sitter, tree-sitter-java
   - stack-graphs, tree-sitter-stack-graphs
   - serde, serde_json, serde_yaml
5. Create basic project structure:
   - `src/main.rs` (copy from C# provider)
   - `src/provider/` directory
   - `src/java_graph/` directory
   - `src/analyzer_service/` directory
6. Verify build: `cargo build`

**Success Criteria**:
- Project builds without errors
- Protobuf compilation works
- Can run empty gRPC server

### Task 1.2: tree-sitter-java Integration

**Objective**: Parse Java source files into ASTs

**Steps**:
1. Create `src/java_graph/language_config.rs`
2. Configure tree-sitter-java parser
3. Write helper function to parse .java file → tree-sitter Tree
4. Create test cases with sample Java files:
   - Simple class with package and imports
   - Class with inheritance
   - Class with method calls
   - Class with annotations
5. Verify AST structure matches expectations

**Test Files to Create**:
- `tests/fixtures/Simple.java` - basic class
- `tests/fixtures/Inheritance.java` - extends clause
- `tests/fixtures/Interfaces.java` - implements clause
- `tests/fixtures/Annotations.java` - JPA annotations

**Success Criteria**:
- Can parse all test files without errors
- Can traverse AST and find expected nodes (class, method, field)
- Tests pass

### Task 1.3: Basic TSG Rules

**Objective**: Create TSG rules for fundamental Java constructs

**Steps**:
1. Create `src/java_graph/stack-graphs.tsg`
2. Write TSG rules for:
   - Package declarations
   - Import statements (regular and wildcard)
   - Class declarations
   - Method declarations
   - Field declarations
3. Create `src/java_graph/loader.rs` to build stack-graph from Java files
4. Test graph building with sample files
5. Verify graph contains expected nodes and edges

**Success Criteria**:
- Stack-graph builds from sample Java files
- Graph contains package, import, class, method, field nodes
- FQDN edges connect symbols to containers
- Can serialize/deserialize graph to SQLite

### Task 1.4: TypeResolver Foundation

**Objective**: Build symbol tables and import resolution

**Steps**:
1. Create `src/java_graph/type_resolver.rs`
2. Define data structures:
   - `SymbolTable` (package, imports, classes per file)
   - `ClassInfo` (name, FQDN, methods, fields)
   - `TypeResolver` (global index, symbol tables per file)
3. Implement `analyze_file()` to extract:
   - Package declaration
   - Import statements (explicit and wildcard)
   - Class definitions
4. Implement `resolve_type_name()` to convert simple names → FQDNs:
   - Check explicit imports
   - Check wildcard imports
   - Check same package
   - Check java.lang (implicit)
5. Write unit tests for type resolution

**Test Cases**:
- Resolve `List` → `java.util.List` (via import)
- Resolve `String` → `java.lang.String` (implicit)
- Resolve `MyClass` → `com.example.MyClass` (same package)
- Wildcard import resolution

**Success Criteria**:
- TypeResolver builds symbol tables from Java files
- Type name resolution works correctly
- All unit tests pass

### Task 1.5: Advanced TSG Rules

**Objective**: Add TSG rules for semantic constructs

**Steps**:
1. Extend `stack-graphs.tsg` with:
   - Inheritance (`extends` clauses) → create inheritance edges
   - Interface implementation (`implements` clauses) → create implements edges
   - Method invocations → create method_call nodes
   - Constructor calls (`new` expressions) → create constructor_call nodes
   - Annotations → create annotation nodes with element support
2. Update `loader.rs` to apply new rules
3. Test with complex Java files

**Test Files**:
- Class with multiple interfaces
- Nested method calls
- Annotations with element values

**Success Criteria**:
- Graph contains inheritance and implements edges
- Method calls and constructor calls tracked
- Annotation nodes created with correct metadata

### Task 1.6: TypeResolver - Inheritance Tracking

**Objective**: Track inheritance and interface relationships

**Steps**:
1. Extend `TypeResolver` with:
   - `inheritance: HashMap<String, String>` (child → parent)
   - `implementations: HashMap<String, Vec<String>>` (class → interfaces)
2. Extract `extends` and `implements` clauses from AST
3. Resolve parent/interface type names to FQDNs
4. Implement transitive queries:
   - `extends_class(class_fqdn, parent_pattern)` - walk up hierarchy
   - `implements_interface(class_fqdn, interface_pattern)` - check all interfaces
5. Write tests for transitive inheritance

**Test Cases**:
- `class B extends A` → `B.extends_class("A")` returns true
- `class C extends B extends A` → `C.extends_class("A")` returns true (transitive)
- `class X implements Serializable` → `X.implements_interface("Serializable")` returns true

**Success Criteria**:
- Inheritance hierarchy tracked correctly
- Interface implementations tracked
- Transitive queries work
- Tests pass

### Task 1.7: Basic Query Engine

**Objective**: Query stack-graph by location type

**Steps**:
1. Create `src/java_graph/query.rs`
2. Define `LocationType` enum (all 15 types)
3. Define `ReferencedCondition` struct (pattern, location, file_paths)
4. Implement FQDN resolution via graph traversal
5. Implement basic pattern matching (regex only for now)
6. Create query function for simple location types:
   - `import`: filter nodes by syntax_type = "import"
   - `package`: filter nodes by syntax_type = "package_declaration"
   - `class`: filter nodes by syntax_type = "class_def"
7. Test queries on sample projects

**Success Criteria**:
- Can query for imports, packages, classes
- FQDN resolution works
- Pattern matching works for regex patterns
- Returns IncidentContext with file locations

---

## Phase 2: Complete "referenced" Capability

### Task 2.1: Pattern Matcher (Wildcard + Regex)

**Objective**: Support literal, wildcard, and regex patterns

**Steps**:
1. Create `src/filter/pattern_matcher.rs`
2. Add `wildmatch` dependency to `Cargo.toml`
3. Implement `PatternMatcher` enum:
   - `Literal(String)` - exact match
   - `Wildcard(WildMatch)` - `org.springframework.*`
   - `Regex(Regex)` - full regex
4. Implement pattern detection logic
5. Write tests for all pattern types

**Test Cases**:
- Literal: `javax.servlet.http.HttpServlet` matches exactly
- Wildcard: `javax.servlet.*` matches `javax.servlet.http.HttpServlet`
- Regex: `javax\.servlet\..*` matches same

**Success Criteria**:
- All three pattern types work
- Tests pass
- Integrated into query engine

### Task 2.2: All Simple Location Types

**Objective**: Implement queries for simple location types

**Steps**:
1. Create individual query files:
   - `src/java_graph/import_query.rs`
   - `src/java_graph/package_query.rs`
   - `src/java_graph/type_query.rs`
   - `src/java_graph/method_query.rs`
   - `src/java_graph/field_query.rs`
   - `src/java_graph/enum_query.rs`
2. Each query filters by syntax_type and matches FQDN against pattern
3. Write tests for each location type

**Success Criteria**:
- All simple location types work
- Tests pass with various patterns
- Results include correct file locations

### Task 2.3: Semantic Location Types (Inheritance & Implements)

**Objective**: Query for inheritance and interface implementation

**Steps**:
1. Create `src/java_graph/inheritance_query.rs`
2. Implement two-strategy approach:
   - Strategy 1: Find nodes with syntax_type = "inheritance" in graph
   - Strategy 2: Use TypeResolver for transitive queries
3. Create `src/java_graph/implements_query.rs`
4. Similar two-strategy approach for interfaces
5. Test with inheritance hierarchies

**Test Cases**:
- Find all classes extending `HttpServlet`
- Find all classes implementing `Serializable` (including via parent)
- Transitive inheritance (3+ levels)

**Success Criteria**:
- Inheritance queries work (direct and transitive)
- Implements queries work (direct and transitive)
- Results include extends/implements clause locations

### Task 2.4: Call Location Types

**Objective**: Implement method_call, constructor_call, variable_declaration, return_type

**Steps**:
1. Create query files for each type
2. `method_call`: filter syntax_type = "method_call", resolve target method FQDN
3. `constructor_call`: filter syntax_type = "constructor_call"
4. `variable_declaration`: filter syntax_type = "variable_declaration"
5. `return_type`: filter syntax_type = "return_type"
6. Test with complex call chains

**Success Criteria**:
- All call-related location types work
- Handles chained method calls
- Returns correct locations

### Task 2.5: Annotation Queries with Element Filtering

**Objective**: Query annotations with element value matching

**Steps**:
1. Create `src/java_graph/annotation_query.rs`
2. Implement annotation extraction from AST
3. Parse element values: `@Table(name = "users")` → `{name: "users"}`
4. Implement element filtering logic
5. Create `src/filter/annotation_filter.rs` for element matching
6. Test with JPA and Spring annotations

**Test Cases**:
- Find all `@Entity` annotations
- Find `@Table(name = "users")` specifically
- Find annotations with array elements

**Success Criteria**:
- Annotation queries work
- Element filtering works
- Complex annotations handled (nested, arrays)

### Task 2.6: File Path Filtering

**Objective**: Support file path include/exclude patterns

**Steps**:
1. Add file path filtering to query engine
2. Support glob patterns (e.g., `src/main/java/**/*.java`)
3. Test with multi-module projects

**Success Criteria**:
- Can filter results by file path
- Glob patterns work
- Include/exclude logic correct

### Task 2.7: Code Snippet Extraction

**Objective**: Extract code snippets with context lines

**Steps**:
1. Create `src/provider/snipper.rs` (reference C# provider)
2. Implement snippet extraction:
   - Read source file
   - Extract lines around incident location
   - Add line numbers (right-aligned)
3. Make context lines configurable (default 10)
4. Test with various locations

**Success Criteria**:
- Snippets extracted correctly
- Line numbers formatted properly
- Context lines configurable

### Task 2.8: gRPC Service - Referenced Capability

**Objective**: Wire up "referenced" capability in gRPC service

**Steps**:
1. Create `src/provider/java.rs` (reference C# provider structure)
2. Implement `capabilities()` RPC → return `["referenced"]`
3. Implement `init()` RPC:
   - Discover .java files
   - Build stack-graph
   - Build TypeResolver
   - Persist to SQLite
4. Implement `evaluate()` RPC:
   - Parse condition YAML
   - Route to appropriate query by location type
   - Return IncidentContext results
5. Implement `notify_file_changes()` RPC:
   - Update graph incrementally
   - Rebuild TypeResolver for changed files

**Success Criteria**:
- All RPCs implemented
- Can initialize and query Java projects
- File change notifications work

### Task 2.9: Integration Testing - Referenced

**Objective**: End-to-end tests for "referenced" capability

**Steps**:
1. Create test scenarios in `e2e-tests/`:
   - Find javax.servlet.* imports
   - Find classes extending HttpServlet
   - Find @Entity annotations
   - Find method calls to specific APIs
2. Test with Spring PetClinic project
3. Verify all 15 location types work
4. Compare results with Go provider (if possible)

**Success Criteria**:
- All E2E tests pass
- Results match expected patterns
- Performance acceptable

---

## Phase 3: "dependency" Capability

### Task 3.1: Build Tool Detection

**Objective**: Detect Maven and Gradle projects

**Steps**:
1. Create `src/buildtool/detector.rs`
2. Implement detection logic:
   - Check for `pom.xml` → Maven
   - Check for `build.gradle` or `build.gradle.kts` → Gradle
   - Find Maven/Gradle executables (mvn, gradle, wrappers)
3. Create `src/provider/project.rs` for project metadata
4. Test with sample Maven and Gradle projects

**Success Criteria**:
- Correctly detects Maven projects
- Correctly detects Gradle projects
- Finds executables (including wrappers)

### Task 3.2: Maven Integration

**Objective**: Extract Maven dependency trees

**Steps**:
1. Create `src/buildtool/maven.rs`
2. Add `quick-xml` dependency for pom.xml parsing
3. Implement `get_dependency_tree()`:
   - Execute `mvn dependency:tree -DoutputType=text`
   - Parse tree-structured output
   - Build `DepDAG` structure
4. Implement pom.xml fallback:
   - Parse `<dependencies>` section with quick-xml
   - Extract groupId, artifactId, version
5. Test with various Maven projects

**Test Cases**:
- Simple Maven project
- Multi-module Maven project
- Maven project with parent POM

**Success Criteria**:
- Dependency tree extracted correctly
- Transitive dependencies included
- Fallback parsing works
- Tests pass

### Task 3.3: Gradle Integration

**Objective**: Extract Gradle dependency trees

**Steps**:
1. Create `src/buildtool/gradle.rs`
2. Implement `get_dependency_tree()`:
   - Execute `./gradlew dependencies --configuration=compileClasspath`
   - Parse indentation-based tree output
   - Build `DepDAG` structure
3. Detect Gradle version and handle differences (9.0+ changes)
4. Support multi-project builds (`./gradlew projects`)
5. Test with Gradle projects

**Test Cases**:
- Simple Gradle project
- Gradle multi-module project
- Gradle 9.0+ project

**Success Criteria**:
- Dependency tree extracted correctly
- Version detection works
- Multi-module support
- Tests pass

### Task 3.4: Dependency Caching

**Objective**: Cache dependency trees to avoid repeated extraction

**Steps**:
1. Create `src/buildtool/dep_cache.rs`
2. Add `sha2` dependency for SHA256 hashing
3. Implement cache:
   - Hash build file content (pom.xml or build.gradle)
   - Use hash as cache key
   - Store dependency tree in cache
4. Implement lock-based concurrency control
5. Test cache hit/miss scenarios

**Success Criteria**:
- Cache hit avoids re-extraction
- Cache invalidated when build file changes
- Concurrent access handled safely

### Task 3.5: Dependency Filtering

**Objective**: Filter dependencies by pattern and version bounds

**Steps**:
1. Create `src/dependency/analyzer.rs`
2. Implement pattern matching for Maven coordinates:
   - `groupId:artifactId`
   - `groupId:artifactId:version`
   - Regex patterns for flexible matching
3. Implement version bounds checking:
   - `lowerbound` and `upperbound` constraints
4. Implement transitive dependency traversal
5. Test with various patterns

**Test Cases**:
- Find `org.springframework:*` dependencies
- Find dependencies with version constraints
- Transitive dependency filtering

**Success Criteria**:
- Pattern matching works for coordinates
- Version bounds checking works
- Transitive traversal correct

### Task 3.6: gRPC Service - Dependency Capability

**Objective**: Wire up "dependency" capability in gRPC service

**Steps**:
1. Update `capabilities()` → return `["referenced", "dependency"]`
2. Implement `evaluate_dependency()` in `src/provider/java.rs`:
   - Get dependency tree from build tool
   - Filter by pattern
   - Convert to IncidentContext (one per dependency)
3. Test with dependency rules

**Success Criteria**:
- Dependency capability works
- Returns matching dependencies
- Results in correct format

### Task 3.7: Integration Testing - Dependency

**Objective**: End-to-end tests for "dependency" capability

**Steps**:
1. Create dependency test scenarios
2. Test with Maven and Gradle projects
3. Test multi-module projects
4. Verify transitive dependencies

**Success Criteria**:
- All dependency tests pass
- Multi-module projects work
- Results accurate

---

## Phase 4: Binary Artifacts & Full Analysis Mode

### Task 4.1: Maven Settings Generation

**Objective**: Generate Maven settings.xml with proxy configuration

**Steps**:
1. Create `src/buildtool/settings.rs`
2. Implement settings.xml generation:
   - Local repository path
   - Proxy configuration (HTTP/HTTPS)
   - NoProxy list
   - Credentials handling
3. Write to `~/.analyze/globalSettings.xml`
4. Test with proxy configurations

**Success Criteria**:
- Settings.xml generated correctly
- Proxy configuration works
- Maven uses generated settings

### Task 4.2: Source Resolution

**Objective**: Download source JARs for dependencies

**Steps**:
1. Implement Maven source download:
   - `mvn de.qaware.maven:go-offline-maven-plugin:resolve-dependencies`
2. Implement Gradle source download:
   - Generate custom task
   - Handle Gradle 9.0+ (no --build-file flag)
3. Track unresolved sources for decompilation
4. Test with real projects

**Success Criteria**:
- Source JARs downloaded
- Unresolved artifacts identified
- Works with Maven and Gradle

### Task 4.3: FernFlower Decompilation

**Objective**: Decompile binary JARs without sources

**Steps**:
1. Create `src/dependency/decompiler.rs`
2. Add `tempfile` dependency
3. Implement worker pool pattern:
   - Spawn 10 concurrent FernFlower processes
   - Queue decompilation jobs
   - Handle failures gracefully
4. Execute: `java -jar fernflower.jar -mpm=30 <input> <output>`
5. Test with sample JARs

**Success Criteria**:
- Decompilation works
- Worker pool functional
- Failures handled
- Output in correct location

### Task 4.4: Artifact Identification

**Objective**: Identify Maven coordinates for binary artifacts

**Steps**:
1. Create `src/dependency/artifact.rs`
2. Add `reqwest` dependency for HTTP requests
3. Implement 3-strategy identification:
   - Strategy 1: SHA1 lookup via Maven Central API
   - Strategy 2: Read `META-INF/maven/*/pom.properties`
   - Strategy 3: Infer from package structure
   - Fallback: Use `EMBEDDED_KONVEYOR_GROUP`
4. Test with various JAR files

**Success Criteria**:
- Coordinates identified correctly
- All strategies work
- Fallback handles unknown artifacts

### Task 4.5: Archive Handlers (JAR/WAR/EAR)

**Objective**: Extract and organize binary artifacts

**Steps**:
1. Create `src/dependency/jar.rs`, `war.rs`, `ear.rs`
2. Add `zip` dependency for archive extraction
3. Implement JAR handler:
   - Extract to temporary directory
   - Create Maven structure (`src/main/java/`)
   - Organize by package
4. Implement WAR handler:
   - `WEB-INF/classes/` → `src/main/java/`
   - `WEB-INF/lib/` → dependencies
5. Implement EAR handler:
   - Multi-module structure
   - Nested archives
6. Test with real artifacts

**Success Criteria**:
- JAR/WAR/EAR extraction works
- Maven structure created correctly
- Nested archives handled

### Task 4.6: Full Analysis Mode

**Objective**: Integrate source resolution + decompilation

**Steps**:
1. Update `init()` RPC to support "full" analysis mode
2. Pipeline:
   - Extract dependency tree
   - Resolve sources from Maven/Gradle
   - Identify unresolved artifacts
   - Decompile with FernFlower
   - Re-analyze decompiled sources
3. Test with projects lacking source JARs

**Success Criteria**:
- Full analysis mode works
- Sources and decompiled code analyzed
- Dependency incidents detected

### Task 4.7: Performance Testing & Optimization

**Objective**: Ensure acceptable performance

**Steps**:
1. Profile with large projects:
   - Spring PetClinic
   - Apache Kafka (if available)
2. Identify bottlenecks
3. Optimize hot paths:
   - Graph building
   - Query execution
   - Dependency tree parsing
4. Set performance targets:
   - Graph build: < 30s for 10k files
   - Query: < 100ms
   - Memory: < 500MB for medium projects
5. Add benchmarks

**Success Criteria**:
- Performance targets met
- No major bottlenecks
- Benchmarks documented

### Task 4.8: Final Integration Testing

**Objective**: Comprehensive end-to-end testing

**Steps**:
1. Test all capabilities together
2. Test with real-world projects
3. Test binary artifacts (JAR/WAR/EAR)
4. Test full analysis mode
5. Compare results with Go provider (manual verification)
6. Document any known limitations

**Success Criteria**:
- All tests pass
- Real-world projects work
- Results accurate
- Limitations documented

---

## Testing Strategy

### Unit Tests
- TypeResolver type resolution
- Pattern matching (literal, wildcard, regex)
- Maven/Gradle tree parsing
- Artifact identification
- Each query type

### Integration Tests
- Stack-graph building from Java files
- Query execution on sample projects
- Dependency tree extraction
- Decompilation pipeline

### E2E Tests
- Full analysis workflow (init → evaluate → results)
- All 15 location types
- Both capabilities (referenced, dependency)
- Binary artifact analysis
- Multi-module projects

### Test Projects
1. **Simple Maven**: Basic project for quick iteration
2. **Spring PetClinic**: Complex Spring Boot app
3. **Gradle Multi-module**: Test Gradle support
4. **Binary Archives**: JAR/WAR/EAR samples
5. **Inheritance Test**: Deep inheritance hierarchy
6. **Annotation Test**: Complex JPA/Spring annotations

---

## Success Criteria

### Phase 1 Success
- ✅ Project builds and runs
- ✅ Parses Java source files
- ✅ Builds stack-graph
- ✅ TypeResolver tracks inheritance/implements
- ✅ Basic queries work

### Phase 2 Success
- ✅ All 15 location types functional
- ✅ Pattern matching (literal, wildcard, regex)
- ✅ Annotation filtering with elements
- ✅ "referenced" capability complete
- ✅ E2E tests pass

### Phase 3 Success
- ✅ Maven dependency trees extracted
- ✅ Gradle dependency trees extracted
- ✅ Dependency caching works
- ✅ "dependency" capability complete
- ✅ Multi-module projects supported

### Phase 4 Success
- ✅ Source resolution works
- ✅ FernFlower decompilation works
- ✅ Binary artifacts (JAR/WAR/EAR) analyzed
- ✅ Full analysis mode operational
- ✅ Performance acceptable
- ✅ All tests pass

### Final Success
- ✅ 100% API compatibility with Go provider
- ✅ No language server dependency
- ✅ Self-contained Rust binary
- ✅ Comprehensive test coverage
- ✅ Documentation complete

---

## Notes

### Key Principles
- **Test-driven**: Write tests as you build features
- **Incremental**: Build in small, verifiable steps
- **Reference C# provider**: Reuse proven patterns
- **Verify continuously**: Test against sample projects frequently

### When to Ask for Help
- TSG rules not producing expected graph
- Type resolution edge cases
- Performance issues
- API compatibility questions

### Documentation to Maintain
- Architecture notes (as you learn)
- TSG rule documentation
- Known limitations
- Testing guide

---

## References

- **Design Document**: `java-provider-design.md`
- **C# Provider**: `/home/jmle/Dev/redhat/c-sharp-analyzer-provider`
- **Go Java Provider**: https://github.com/konveyor/analyzer-lsp/tree/main/external-providers/java-external-provider
- **tree-sitter-java**: https://github.com/tree-sitter/tree-sitter-java
- **stack-graphs**: https://github.com/github/stack-graphs
