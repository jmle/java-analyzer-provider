# Java Analyzer Provider

A high-performance Rust-based Java static code analyzer provider for the Konveyor migration platform.

## Overview

This provider delivers fast, accurate Java code analysis without requiring JDTLS (Eclipse Language Server) or a JVM runtime. It uses:

- **tree-sitter-java**: Fast, incremental Java parsing
- **stack-graphs**: GitHub's semantic code navigation technology
- **TypeResolver**: Custom semantic layer for inheritance/implements tracking
- **Maven/Gradle integration**: Full dependency tree analysis
- **Pure Rust**: Self-contained binary with minimal dependencies

## Architecture

```
tree-sitter-java → AST
       ↓
TSG Rules → Stack Graph (semantic nodes + edges)
       ↓
TypeResolver (inheritance, implements, imports)
       ↓
Query Engine (15 location types)
       ↓
gRPC Service (referenced + dependency capabilities)
```

## Capabilities

### 1. "referenced" - 15 Location Types

- `type` - Type references
- `inheritance` - Class extends relationships
- `implements_type` - Interface implementations
- `method_call` - Method invocations
- `constructor_call` - Constructor calls
- `annotation` - Java annotations (with element filtering)
- `return_type` - Return type declarations
- `import` - Import statements
- `variable_declaration` - Variable declarations
- `package` - Package declarations
- `field` - Field declarations
- `method` - Method declarations
- `class` - Class declarations
- `enum` - Enum declarations

### 2. "dependency" - Dependency Tree Analysis

- Maven dependency trees (`mvn dependency:tree`)
- Gradle dependency trees (`./gradlew dependencies`)
- Transitive dependency traversal
- Version constraint matching
- Multi-module project support

## Quick Start

### Using Docker/Podman (Recommended)

```bash
# Build image
make build-image

# Run container
make run-container

# Test in container
make test-docker
```

Or manually:
```bash
podman build -f Dockerfile -t java-provider:latest .
podman run --rm -p 9000:9000 java-provider:latest
```

### Building from Source

#### Prerequisites

- Rust 1.75+ (cargo, rustc)
- Java 17+ (OpenJDK)
- Maven 3.6+ (optional, for dependency resolution)
- Gradle 8+ (optional, for dependency resolution)
- protoc (for gRPC development)

#### Build

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test
```

#### Run

```bash
# Start the gRPC server on port 9000
cargo run -- 9000

# Or use the compiled binary
./target/release/java-analyzer-provider 9000
```

## Docker Deployment

### Production Image

The production Dockerfile creates an optimized multi-stage build:

```bash
# Build
podman build -f Dockerfile -t java-provider:latest .

# Run
podman run --rm -p 9000:9000 java-provider:latest

# With volume mount
podman run --rm -p 9000:9000 \
  -v /path/to/java/project:/analyzer-lsp/project:Z \
  java-provider:latest
```

**Image Details**:
- Base: Red Hat UBI 9 (minimal)
- Includes: Java 17, Maven 3.x, Gradle 8.5
- Size: ~1 GB
- User: UID 1001 (non-root, OpenShift compatible)
- Port: 9000 (default)

### Test Image

```bash
# Build and run tests
make build-test-image
make test-docker
```

### Makefile Commands

```bash
make help              # Show all available targets
make build-image       # Build Docker image
make run-container     # Run in container
make test-docker       # Run tests in container
make run-java-pod      # Run in pod with test data
```

For more details, see [docs/docker.md](docs/docker.md).

## Development

### Project Structure

```
src/
├── main.rs                 # Entry point
├── provider/               # gRPC service
│   ├── java.rs            # Main provider implementation
│   ├── project.rs         # Maven/Gradle detection
│   └── snipper.rs         # Code snippet extraction
├── java_graph/            # Core analysis engine
│   ├── loader.rs          # Graph building
│   ├── query.rs           # Query engine
│   ├── type_resolver.rs   # TypeResolver (NEW)
│   ├── *_query.rs         # Individual location type queries
│   └── stack-graphs.tsg   # TSG rules for Java
├── buildtool/             # Maven/Gradle integration
│   ├── maven.rs           # Maven CLI integration
│   ├── gradle.rs          # Gradle CLI integration
│   └── dep_cache.rs       # Dependency caching
├── dependency/            # Binary artifact support
│   ├── decompiler.rs      # FernFlower integration
│   ├── artifact.rs        # Maven coordinate identification
│   └── jar.rs, war.rs     # Archive handlers
└── filter/                # Pattern matching
    └── pattern_matcher.rs # Literal/Wildcard/Regex
```

## Testing

```bash
# Run unit tests
cargo test

# Run integration tests
cargo test --test '*'

# E2E tests (full integration with konveyor-analyzer)
make e2e-setup     # Download konveyor-analyzer (first time only)
make e2e           # Run E2E tests and verify output
make e2e-local     # Run E2E tests only (skip verification)
```

See [e2e-tests/README.md](e2e-tests/README.md) for comprehensive E2E testing guide.

## Documentation

- [Docker Guide](docs/docker.md) - Comprehensive Docker/Podman deployment guide
- [E2E Testing](e2e-tests/README.md) - End-to-end testing with konveyor-analyzer

## Features

- ✅ Full gRPC ProviderService implementation
- ✅ 15 location types supported
- ✅ Annotation filtering with element regex matching
- ✅ Maven dependency resolution (pom.xml + mvn dependency:tree)
- ✅ Gradle dependency resolution (build.gradle/.kts + gradle dependencies)
- ✅ Semantic version comparison and range matching
- ✅ Advanced pattern matching (wildcards, regex, AND/OR/NOT)
- ✅ Pattern caching for performance
- ✅ Progress streaming for long operations
- ✅ OpenShift-compatible containers

## Test Coverage

- **191+ unit tests** across all modules
- Integration tests for Maven and Gradle
- Pattern matching and annotation filtering tests
- E2E tests with konveyor-analyzer (34/42 rules passing)

## License

Apache-2.0

## References

- C# Provider: [../c-sharp-analyzer-provider](../c-sharp-analyzer-provider)
- Go Java Provider: https://github.com/konveyor/analyzer-lsp/tree/main/external-providers/java-external-provider
- tree-sitter-java: https://github.com/tree-sitter/tree-sitter-java
- stack-graphs: https://github.com/github/stack-graphs
