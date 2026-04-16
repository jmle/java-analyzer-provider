# Java Provider Design for Konveyor: Analysis and Prototype Proposal

**Date**: April 13, 2026  
**Author**: Claude Code Analysis  
**Purpose**: Design document for creating a self-contained Rust-based Java static code analyzer provider, eliminating language server dependencies

**Status**: Updated with no-language-server constraint

---

## Table of Contents

1. [Executive Summary](#executive-summary)
2. [Existing Java Provider Analysis](#existing-java-provider-analysis)
3. [C# Provider Architecture Analysis](#c-provider-architecture-analysis)
4. [Java Provider Design Proposal](#java-provider-design-proposal)
5. [Technology Stack Comparison](#technology-stack-comparison)
6. [Implementation Roadmap](#implementation-roadmap)
7. [Challenges and Mitigations](#challenges-and-mitigations)
8. [Appendix: Key Design Decisions](#appendix-key-design-decisions)

---

## Executive Summary

### Project Goal

Create a **self-contained Rust-based Java analyzer provider** that:
- ✅ **Eliminates language server dependency** (no JDTLS, no LSP)
- ✅ **Maintains API compatibility** with existing Java provider
- ✅ **Supports all 15 location types** for the "referenced" capability
- ✅ **Implements full "dependency" capability** with Maven/Gradle integration
- ✅ **Follows C# provider architecture** (tree-sitter + stack-graphs)
- ✅ **Self-contained**: No external Java processes required for analysis

### Key Architectural Decision

**Use tree-sitter + stack-graphs + custom semantic layer** (pure Rust, no language server)

```
┌─────────────────────────────────────────────────────────┐
│        Java Provider (Pure Rust, Self-Contained)        │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐  │
│  │ gRPC Service │  │ Semantic     │  │ Graph Loader │  │
│  │              │  │ Analyzer     │  │ (tree-sitter)│  │
│  └──────┬───────┘  └──────┬───────┘  └──────┬───────┘  │
│         │                 │                 │          │
│         ▼                 ▼                 ▼          │
│  ┌─────────────────────────────────────────────────┐   │
│  │        Stack Graph + Type Resolver              │   │
│  │  - Symbol tables (classes, methods, fields)     │   │
│  │  - Inheritance hierarchy                        │   │
│  │  - Type resolution across files                 │   │
│  └─────────────────────────────────────────────────┘   │
│         │                                               │
│         ▼                                               │
│  ┌─────────────────────────────────────────────────┐   │
│  │        BuildTool (Maven/Gradle)                 │   │
│  │  - Dependency tree extraction (CLI)             │   │
│  │  - FernFlower decompilation (external tool)     │   │
│  └─────────────────────────────────────────────────┘   │
└─────────────────────────────────────────────────────────┘
```

### Why This Approach Works

**All 15 location types can be supported without a language server**:

| Location Type | Analysis Method | Feasibility |
|---------------|----------------|-------------|
| `type` | tree-sitter AST (type references) | ✅ Easy |
| `inheritance` | TSG rules + extends clause tracking | ✅ Moderate |
| `implements_type` | TSG rules + implements clause tracking | ✅ Moderate |
| `method_call` | tree-sitter AST (method invocations) | ✅ Easy |
| `constructor_call` | tree-sitter AST (new expressions) | ✅ Easy |
| `annotation` | tree-sitter AST (annotation declarations) | ✅ Easy |
| `return_type` | tree-sitter AST (method return types) | ✅ Easy |
| `import` | tree-sitter AST (import statements) | ✅ Easy |
| `variable_declaration` | tree-sitter AST (variable declarations) | ✅ Easy |
| `package` | tree-sitter AST (package declarations) | ✅ Easy |
| `field` | tree-sitter AST (field declarations) | ✅ Easy |
| `method` | tree-sitter AST (method declarations) | ✅ Easy |
| `class` | tree-sitter AST (class declarations) | ✅ Easy |
| `enum` | tree-sitter AST (enum declarations) | ✅ Easy |

**Key insight**: Stack-graphs are designed to resolve symbols across files, handle scopes, and build semantic relationships. Combined with comprehensive TSG rules and a type resolution layer, we can achieve the same semantic depth as JDTLS for migration analysis use cases.

### What We Gain by Eliminating Language Servers

1. **Simplicity**: No subprocess management, no JSON-RPC, no JDTLS lifecycle
2. **Performance**: No JVM startup time, no LSP initialization overhead
3. **Reliability**: No language server crashes, no protocol mismatches
4. **Portability**: Pure Rust binary, no Java runtime requirement
5. **Consistency**: Same architecture as C# provider (easier maintenance)
6. **Resource Efficiency**: Lower memory footprint (no JVM heap)

### Implementation Estimate

**10-12 weeks** for full compatibility with existing Java provider:
- Weeks 1-4: Core analysis engine (tree-sitter + stack-graphs + TSG rules)
- Weeks 5-7: All 15 location types + semantic layer
- Weeks 8-10: Dependency capability + Maven/Gradle integration
- Weeks 11-12: Binary artifacts + decompilation + polish

---

## Existing Java Provider Analysis

**What it does**:
- **Language**: Go (~15,000 lines)
- **Analysis Engine**: JDTLS (Eclipse Language Server) - **requires JVM**
- **2 Capabilities**:
  1. **"referenced"**: 15 location types (type, inheritance, implements_type, method_call, constructor_call, annotation, return_type, import, variable_declaration, package, field, method, class, enum)
  2. **"dependency"**: Maven/Gradle dependency tree analysis with transitive traversal
- **Patterns**: Literal, wildcard (`org.springframework.*`), and regex
- **Binary Support**: JAR/WAR/EAR with FernFlower decompilation

**What we can reuse** (no language server needed):
- ✅ Maven/Gradle CLI integration (`mvn dependency:tree`, `./gradlew dependencies`)
- ✅ Dependency tree parsing logic
- ✅ FernFlower decompilation (external tool)
- ✅ Artifact identification (SHA1 lookup, pom.properties)
- ✅ Code snippet extraction
- ✅ Pattern matching logic

**What we must replace** (language server specific):
- ❌ `service_client.go` (1,500 lines) - JDTLS JSON-RPC client
- ❌ Symbol queries via JDTLS (`io.konveyor.tackle.ruleEntry`)

**Replacement strategy**: tree-sitter-java + stack-graphs + TypeResolver (custom semantic layer)

---

## C# Provider Architecture Analysis

**What it is**:
- **Self-contained Rust provider** (10,457 lines, no language server)
- **Architecture**: tree-sitter-c-sharp + stack-graphs + SQLite
- **Capabilities**: 1 ("referenced" with 5 location types: All, Method, Field, Class, Namespace)

**Key Technologies**:
- **tree-sitter**: Parses C# source → AST
- **stack-graphs**: GitHub's library for semantic symbol resolution across files
- **TSG rules**: Domain-specific rules mapping syntax → semantic graph
- **SQLite**: Persists graph for fast startup + incremental updates

**How stack-graphs work**:
1. **Nodes**: Symbol definitions (class, method), references (method calls), scopes
2. **Edges**: FQDN edges link symbols to containers (method → class → namespace)
3. **Query**: Filter nodes by type → traverse FQDN edges → match pattern → return locations

**Example query**: Pattern `System.Web.Mvc.*`
```
1. Find nodes with syntax_type = "method_call"
2. Traverse edges: method_call → class → namespace
3. Build FQDN: "System.Web.Mvc.Controller.View"
4. Match pattern ✓ → Return file location
```

**Why this architecture works without language server**:
- Stack-graphs handle cross-file symbol resolution
- TSG rules provide semantic mappings (not just syntax)
- No need for compiler or language server for migration analysis use cases

---

## Java Provider Design Proposal

### Architecture: Pure Rust, Self-Contained

**High-level approach**: Extend C# provider architecture with Java-specific semantic layer

**Core Components**:

1. **Analysis Layer** (tree-sitter-java + stack-graphs)
   - Parse `.java` files → AST
   - Apply Java TSG rules → stack-graph
   - Track symbols, scopes, references across files
   - Persist to SQLite

2. **Type Resolver** (NEW - Java semantic layer)
   - **Symbol tables per file**: Track packages, imports, classes, methods, fields
   - **Import resolution**: Convert simple names → FQDNs (e.g., `List` → `java.util.List`)
   - **Inheritance tracking**: Build parent-child relationships from `extends` clauses
   - **Interface tracking**: Track `implements` clauses
   - **Annotation metadata**: Extract annotation names + element values
   - **Transitive queries**: Walk inheritance hierarchy for `inheritance` and `implements_type` location types

3. **Query Engine** (from C# provider + extensions)
   - Route by location type (15 types total)
   - Filter stack-graph nodes by syntax type
   - Resolve FQDNs via graph traversal
   - Pattern matching: literal, wildcard (`org.springframework.*`), regex
   - Annotation filtering with element matching

4. **BuildTool Layer** (reuse from Go provider)
   - Maven: Execute `mvn dependency:tree`, parse output, fallback to pom.xml parsing
   - Gradle: Execute `./gradlew dependencies`, parse output, handle version differences
   - Dependency caching (SHA256 of build files)
   - FernFlower decompilation (worker pool, external tool)

5. **gRPC Service** (from C# provider)
   - Init: Discover .java files, build graph, detect Maven/Gradle
   - Evaluate: Route to referenced/dependency handlers
   - NotifyFileChanges: Incremental graph updates


### Technology Stack

**Core Dependencies**:
- **Rust** + Tokio (async runtime) + Tonic (gRPC)
- **tree-sitter-java** v0.23.4: Java parsing
- **stack-graphs** v0.14.1: Semantic analysis
- **tree-sitter-stack-graphs** v0.10.0: TSG rules execution
- **SQLite**: Graph + symbol table persistence

**New Dependencies** (vs. C# provider):
- **wildmatch**: Wildcard pattern matching (`org.springframework.*`)
- **quick-xml**: Maven pom.xml parsing (fallback)
- **zip**: JAR/WAR/EAR extraction
- **sha2**: Dependency cache (SHA256 hashing)
- **reqwest**: Maven Central API (artifact identification)

### Implementation Notes

**How it achieves 100% API compatibility without language server**:

1. **15 Location Types Support**:
   - Simple types (import, package, type, class, method, field, enum): tree-sitter AST directly
   - Semantic types (inheritance, implements_type): TypeResolver + TSG rules tracking extends/implements clauses
   - Call types (method_call, constructor_call): tree-sitter AST for invocations
   - Annotation types: tree-sitter AST + element value extraction

2. **Type Resolution Strategy**:
   - Extract imports per file → build mapping (simple name → FQDN)
   - Example: `List` → check imports → `java.util.List`
   - Wildcards: `import java.util.*` → check type index for `java.util.X`
   - Implicit: `String` → `java.lang.String` (always available)

3. **Inheritance/Interface Tracking**:
   - TSG rules extract `extends` and `implements` clauses from AST
   - TypeResolver builds parent-child maps
   - Transitive queries: walk up inheritance chain to match patterns
   - Example: Find all HttpServlet subclasses → traverse extends edges

4. **Annotation Filtering**:
   - TSG rules create annotation nodes from AST
   - Extract element values: `@Table(name = "users")` → `{name: "users"}`
   - Match both annotation name AND element values

5. **Dependency Analysis** (reuse from Go provider):
   - Execute `mvn dependency:tree` or `./gradlew dependencies` (CLI)
   - Parse text output into DAG structure
   - Fallback: Parse pom.xml/build.gradle with quick-xml
   - Cache with SHA256 of build file

6. **Binary Artifact Support** (reuse from Go provider):
   - Identify coordinates: SHA1 lookup → pom.properties → package inference
   - Decompile with FernFlower (external tool, worker pool)
   - Create Maven structure: JAR → src/main/java/, WAR → extract WEB-INF/

**TSG Rules Coverage** (Java-specific semantic mappings):
- Package declarations → namespace nodes
- Import statements → reference nodes with FQDN edges
- Class declarations → class nodes with scope
- Inheritance clauses → `extends` edges to parent
- Interface implementations → `implements` edges to interfaces
- Method/field declarations → symbol definition nodes
- Method/constructor calls → reference nodes with call edges
- Annotations → annotation nodes with element metadata
- Inner classes, generics, lambdas → nested scope handling

**Estimated Size**: ~14,000-16,000 lines of Rust (similar to Go provider)

---
## Technology Stack Comparison

| Component | C# Provider | Go Java Provider | **New Rust Java Provider** |
|-----------|-------------|------------------|----------------------------|
| **Language** | Rust | Go | Rust |
| **gRPC Framework** | Tonic | grpc-go | Tonic |
| **Async Runtime** | Tokio | Goroutines | Tokio |
| **Analysis Engine** | tree-sitter + stack-graphs | **JDTLS (LSP)** | **tree-sitter + stack-graphs** |
| **Semantic Layer** | Stack-graphs TSG | JDTLS | **Stack-graphs + TypeResolver** |
| **Java Runtime** | Not required | **Required** | **Not required** |
| **Process Management** | None | os/exec (for JDTLS) | **tokio::process (for Maven/Gradle only)** |
| **Pattern Matching** | Regex only | Regex + Wildcards | **Regex + Wildcards** |
| **XML Parsing** | None | encoding/xml | **quick-xml** |
| **Archive Handling** | None | archive/zip | **zip crate** |
| **Decompiler** | None | FernFlower (external) | **FernFlower (external)** |
| **Dependency Tree** | Limited | Full Maven/Gradle | **Full Maven/Gradle** |
| **Caching** | SQLite graph | Dependency tree SHA256 | **SQLite graph + Dependency tree** |
| **Location Types** | 5 | 15 | **15** |
| **Capabilities** | 1 (referenced) | 2 (referenced, dependency) | **2 (referenced, dependency)** |
| **Binary Support** | None | JAR/WAR/EAR | **JAR/WAR/EAR** |
| **Self-Contained** | ✅ Yes | ❌ No (needs JVM) | ✅ **Yes** |

**Key Differences**:
- ❌ **Go provider**: Requires JVM for JDTLS, subprocess management complexity
- ✅ **New Rust provider**: Pure Rust, no language server, self-contained binary

---

## Implementation Roadmap

### Phase 1: Core Analysis Engine

**Goal**: Build foundation - stack-graph + TypeResolver + basic queries

**What to build**:
- Project setup (Rust project, dependencies, protobuf from C# provider)
- tree-sitter-java integration (parser configuration, AST verification)
- Basic TSG rules (packages, imports, classes, methods, fields)
- TypeResolver foundation (SymbolTable, import extraction, type resolution)
- Advanced TSG rules (inheritance/implements clauses, annotations, method calls)
- TypeResolver completion (inheritance tracking, interface tracking, transitive resolution)
- Testing infrastructure (unit tests, integration tests with sample projects)

**Success criteria**:
- Stack-graph builds from Java source files
- TypeResolver tracks inheritance and implements relationships
- Can query basic symbols (packages, classes, methods)
- Tests pass on sample Java projects

### Phase 2: Complete "referenced" Capability

**Goal**: Support all 15 location types with full pattern matching

**What to build**:
- Query engine (routing by location type, FQDN resolution, pattern matcher for literal/wildcard/regex)
- Simple location queries (import, package, type, class, method, field, enum)
- Semantic location queries (inheritance, implements_type using TypeResolver)
- Call location queries (method_call, constructor_call, variable_declaration, return_type)
- Annotation filtering (element value extraction and matching)
- File path filtering (glob patterns, include/exclude)
- Code snippet extraction with context lines
- Comprehensive tests for all location types

**Success criteria**:
- All 15 location types functional
- Pattern matching works (literal, wildcard, regex)
- Annotation filtering with element matching
- File path filtering operational
- E2E tests pass with real Java projects (Spring PetClinic, etc.)

### Phase 3: "dependency" Capability

**Goal**: Maven/Gradle dependency tree analysis

**What to build**:
- Build tool detection (Maven pom.xml, Gradle build.gradle, executable discovery)
- Maven integration (execute `mvn dependency:tree`, parse output, pom.xml fallback)
- Gradle integration (execute `./gradlew dependencies`, parse output, version handling)
- Dependency caching (SHA256 hashing, lock-based cache)
- Dependency filtering (pattern matching, version bounds, transitive traversal)
- evaluate_dependency implementation (convert dependencies to IncidentContext)
- Multi-module project support
- Tests with Maven and Gradle projects

**Success criteria**:
- Maven dependency trees extracted and parsed
- Gradle dependency trees extracted and parsed
- Dependency caching functional
- Pattern matching and version filtering work
- Multi-module projects supported

### Phase 4: Binary Artifacts + Full Analysis Mode

**Goal**: JAR/WAR/EAR support with decompilation

**What to build**:
- Maven settings generation (settings.xml with proxy configuration)
- Source resolution (Maven/Gradle source downloads)
- FernFlower integration (worker pool decompiler, subprocess management)
- Artifact identification (SHA1 lookup, pom.properties parsing, package inference)
- Archive handlers (JAR/WAR/EAR extraction and Maven structure creation)
- Full analysis mode (source resolution + decompilation pipeline)
- Performance testing and optimization
- Complete documentation

**Success criteria**:
- Full analysis mode operational
- Binary artifacts (JAR/WAR/EAR) analyzed successfully
- FernFlower decompilation works with worker pool
- Performance acceptable for large projects
- All integration tests pass

## Challenges and Mitigations

### 1. TSG Rule Complexity

**Challenge**: Java has complex semantics (generics, inner classes, lambdas, annotations with elements)

**Mitigation**:
- Start with simple cases (top-level classes, basic methods)
- Iterate to cover edge cases
- Reference tree-sitter-java grammar documentation
- Test extensively with real-world Java projects
- Document known limitations

**Risk level**: Medium

**Estimated effort**: 2-3 weeks for comprehensive TSG rules

### 2. Type Resolution Accuracy

**Challenge**: Resolving simple type names to FQDNs without full compiler

**Mitigation**:
- Handle common cases: explicit imports, same package, java.lang
- Wildcard import resolution (check against type index)
- Best-effort approach: if can't resolve, use simple name
- For migration use cases, most types are explicitly imported

**Risk level**: Medium

**Limitations**:
- May not resolve types from wildcard imports if not in type index
- Won't handle complex generic type inference
- Acceptable for migration analysis (patterns target explicit types)

### 3. Inheritance Hierarchy Completeness

**Challenge**: Building complete inheritance hierarchy requires analyzing all classes including JDK

**Mitigation**:
- Focus on source code classes (not JDK)
- For JDK classes (e.g., HttpServlet), rely on explicit extends clauses
- Don't need to analyze JDK source (too expensive)
- Pattern matching on FQDN is sufficient for migration rules

**Risk level**: Low

**Example**:
```java
// Rule: Find classes extending HttpServlet
// Pattern: javax.servlet.http.HttpServlet

public class MyServlet extends HttpServlet { ... }
```

The `extends HttpServlet` clause is explicit in source, we just need to resolve `HttpServlet` → `javax.servlet.http.HttpServlet` via imports.

### 4. Annotation Element Parsing

**Challenge**: Annotations can have complex element structures (arrays, nested annotations)

**Mitigation**:
- Start with simple element types (string, number)
- Extend to arrays and nested annotations incrementally
- Use tree-sitter AST to extract element values
- Test with common annotations (JPA, Spring)

**Risk level**: Medium

**Example**:
```java
@Table(name = "users", indexes = { @Index(name = "idx", columnList = "id") })
```

Extract:
- `name` → `"users"`
- `indexes` → array of nested annotations (handle recursively)

### 5. Performance at Scale

**Challenge**: Large projects (100k+ lines) may stress graph building

**Mitigation**:
- Profile early with large projects
- Optimize SQLite persistence (batch inserts, indexes)
- Incremental updates (only changed files)
- Parallelize file parsing (Tokio tasks)

**Risk level**: Low

**Benchmarks to target**:
- Graph build: < 30 seconds for 10k .java files
- Query execution: < 100ms for pattern matching
- Memory: < 500MB for medium projects

### 6. Maven/Gradle CLI Reliability

**Challenge**: Build tool CLIs may fail, output formats vary

**Mitigation**:
- Robust error handling
- Fallback to pom.xml parsing for Maven
- Test with multiple Maven/Gradle versions
- Document required versions

**Risk level**: Low (Go provider has proven this works)

### 7. Maintaining API Compatibility

**Challenge**: Must match existing Java provider API exactly

**Mitigation**:
- Share protobuf definitions
- Test against same rule files as Go provider
- Compare output formats
- Regular integration testing with analyzer-lsp

**Risk level**: Very Low (well-defined interface)

---

## Appendix: Key Design Decisions

### Decision 1: No Language Server

**What**: Use pure Rust with tree-sitter + stack-graphs (no JDTLS)

**Why**:
- Eliminate language server dependency (no JVM required)
- Self-contained single Rust binary, easier to distribute
- Better performance (no JVM startup, no LSP protocol overhead)
- Consistency with C# provider architecture
- Easier maintenance (pure Rust vs polyglot system)

**Tradeoff**: More custom code (TSG rules + TypeResolver), but achieves same results for migration use cases

### Decision 2: Stack-Graphs for Semantic Analysis

**What**: Use tree-sitter + stack-graphs (extend with TypeResolver for Java-specific semantics)

**Why**:
- Proven technology (used by GitHub for code navigation)
- Designed for cross-file analysis (handles scopes, references, definitions)
- TSG language provides declarative syntax → semantics mapping
- Reuse from C# provider (same architecture, shared knowledge)

### Decision 3: Add TypeResolver Layer

**What**: Add custom TypeResolver alongside stack-graphs

**Why**:
- Explicit tracking of symbol tables per file
- Dedicated data structures for inheritance/implements queries
- Import resolution mapping (simple names → FQDNs)
- Better performance (in-memory structures vs repeated graph traversal)

### Decision 4: Regex + Wildcards Pattern Matching

**What**: Support literal, wildcard, and regex patterns

**Why**:
- API compatibility with Go provider (supports wildcards)
- User-friendly (wildcards easier for common cases like `org.springframework.*`)
- Library support (`wildmatch` crate provides robust implementation)

### Decision 5: Full Dependency Analysis

**What**: Implement full dependency analysis (not just source-only)

**Why**:
- API compatibility with Go provider
- Some migrations require dependency analysis
- Feasible (Maven/Gradle CLIs provide dependency trees)

**How**: Execute build tool CLIs, parse output (same as Go provider)

### Decision 6: Use FernFlower for Decompilation

**What**: Use FernFlower as decompilation tool (external Java tool)

**Why**:
- Consistency with Go provider
- Proven reliability
- Easy to spawn as subprocess

**How**: Worker pool pattern with 10 concurrent processes

### Decision 7: Cache Both Stack-Graphs and Dependencies

**What**: Use dual caching strategy

**Why**:
- Stack-graph cache enables fast startup (like C# provider)
- Dependency cache avoids expensive `mvn dependency:tree` calls
- Incremental updates (only affected parts rebuild)

**How**:
- SQLite for stack-graphs (built into stack-graphs library)
- SHA256-keyed cache for dependency trees

---
## Conclusion

### Summary

**Goal**: Build a self-contained Rust-based Java analyzer provider without language server dependencies

**Approach**: tree-sitter + stack-graphs + custom type resolver

**Architecture**:
```
Pure Rust Provider
├── tree-sitter-java (parsing)
├── stack-graphs (semantic graph)
├── TypeResolver (inheritance, implements, imports)
├── Maven/Gradle CLI (dependency trees)
└── FernFlower (decompilation)
```

**Key Advantages**:
1. ✅ **No language server**: Eliminates JDTLS dependency
2. ✅ **No JVM required**: Pure Rust binary (except FernFlower for decompilation)
3. ✅ **Self-contained**: Single binary, easy deployment
4. ✅ **Consistent with C# provider**: Same architecture, easier maintenance
5. ✅ **Full API compatibility**: Supports all 15 location types + 2 capabilities
6. ✅ **Performance**: No JVM startup, no LSP overhead
7. ✅ **Maintainable**: Pure Rust, type-safe, well-structured

**Implementation Estimate**: 10-12 weeks
- Weeks 1-4: Core analysis engine
- Weeks 5-7: All 15 location types
- Weeks 8-10: Dependency capability
- Weeks 11-12: Binary artifacts + polish

**Feasibility**: High
- tree-sitter-java is mature
- stack-graphs proven for cross-file analysis
- Type resolver design is straightforward
- Maven/Gradle integration well-understood
- All components have been validated in existing providers

### Next Steps

1. **Prototype type resolver**: Validate import resolution and inheritance tracking
2. **Write initial TSG rules**: Test with sample Java files
3. **Set up project**: Initialize Rust project with dependencies
4. **Implement Phase 1**: Build core analysis engine
5. **Test with real projects**: Spring PetClinic, simple Maven apps

### Resources

**Code References**:
- C# Provider: `/home/jmle/Dev/redhat/c-sharp-analyzer-provider`
- Go Java Provider: https://github.com/konveyor/analyzer-lsp/tree/main/external-providers/java-external-provider

**Technologies**:
- tree-sitter-java: https://github.com/tree-sitter/tree-sitter-java
- stack-graphs: https://github.com/github/stack-graphs
- tree-sitter-stack-graphs: https://docs.rs/tree-sitter-stack-graphs

**Test Projects**:
- Spring PetClinic: https://github.com/spring-projects/spring-petclinic
- Simple Maven App: https://github.com/konveyor/analyzer-lsp/tree/main/external-providers/java-external-provider/examples/java

---

**Document Version**: 3.0  
**Last Updated**: April 13, 2026  
**Status**: Updated with no-language-server constraint  
**Contact**: Claude Code Analysis
