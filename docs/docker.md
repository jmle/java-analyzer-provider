# Docker Setup for Java Analyzer Provider

## Overview

The Java analyzer provider includes Docker support for containerized deployment in Konveyor. This document describes the Docker setup, images, and usage.

## Docker Files

### Dockerfile (Production)

**Purpose**: Production-ready image for deployment

**Base Images**:
- Build stage: `registry.access.redhat.com/ubi9/ubi` (Rust compilation)
- Runtime stage: `registry.access.redhat.com/ubi9/ubi-minimal` (minimal footprint)

**Installed Tools**:
- Java 17 OpenJDK (for running Java applications being analyzed)
- Maven 3.x (for dependency:tree resolution)
- Gradle 8.5 (for dependency resolution)

**Entry Point**: `/usr/local/bin/java-provider`

**Default Port**: 9000 (gRPC)

**User**: Runs as UID 1001 (non-root, OpenShift compatible)

### Dockerfile.test (Testing)

**Purpose**: Testing and development image with additional tools

**Base Image**: `registry.access.redhat.com/ubi9/ubi` (full UBI for development)

**Additional Tools**:
- grpcurl (for testing gRPC endpoints)
- protoc (for protocol buffer compilation)
- git (for source control in tests)
- Development utilities (less, netcat, etc.)

**Entry Point**: `cargo test` (runs test suite by default)

### .dockerignore

Excludes unnecessary files from Docker build context:
- Build artifacts (`target/`)
- Documentation (`docs/`, `*.md`)
- Test fixtures (kept small for faster builds)
- IDE files
- Git metadata

## Building Images

### Build Production Image

```bash
make build-image
# Or manually:
podman build -f Dockerfile -t java-provider:latest .
```

**Image Size**: ~450 MB (after optimization)

**Build Time**: ~5-10 minutes (first build, then cached)

### Build Test Image

```bash
make build-test-image
# Or manually:
podman build -f Dockerfile.test -t java-provider-test:latest .
```

## Running the Container

### Basic Run

```bash
make run-container
# Runs on http://0.0.0.0:9000
```

### Run with Test Data

```bash
make run-container-with-data
# Mounts tests/fixtures/ to /analyzer-lsp/test-data
```

### Manual Run

```bash
podman run --rm -p 9000:9000 java-provider:latest

# With custom port:
podman run --rm -p 14652:14652 java-provider:latest 14652

# With volume mount:
podman run --rm -p 9000:9000 \
  -v /path/to/java/project:/analyzer-lsp/project:Z \
  java-provider:latest
```

## Testing with Docker

### Run Tests in Container

```bash
make test-docker
# Builds test image and runs cargo test
```

### Interactive Shell for Debugging

```bash
podman run --rm -it --entrypoint /bin/bash java-provider:latest

# Inside container:
/usr/local/bin/java-provider 9000 &
grpcurl -plaintext localhost:9000 list
```

## Pod-Based Deployment (Konveyor Integration)

### Create Pod with Provider

```bash
make run-java-pod
# Creates analyzer-java pod with provider running on port 14652
```

### Stop Pod

```bash
make stop-java-pod
```

### Manual Pod Setup

```bash
# Create pod
podman pod create --name=analyzer-java

# Run provider in pod
podman run --pod analyzer-java --name java-provider -d \
  java-provider:latest 14652

# Run analyzer in same pod
podman run --pod analyzer-java \
  --entrypoint /usr/local/bin/konveyor-analyzer \
  -v $(PWD)/provider_settings.json:/analyzer-lsp/provider_settings.json:Z \
  quay.io/konveyor/analyzer-lsp:latest \
  --provider-settings=/analyzer-lsp/provider_settings.json
```

## Testing gRPC Endpoints

### Using grpcurl (Server Running)

```bash
# Start server
make run &

# Wait for server
make wait-for-server

# Test endpoints
make run-grpc-capabilities
make run-grpc-init
make run-grpc-dependencies
make run-grpc-evaluate
```

### Manual grpcurl Commands

```bash
# List services
grpcurl -plaintext localhost:9000 list

# Get capabilities
grpcurl -plaintext localhost:9000 provider.ProviderService.GetCapabilities

# Initialize
grpcurl -plaintext -d '{
  "location": "/analyzer-lsp/test-data",
  "analysisMode": "source-only"
}' localhost:9000 provider.ProviderService.Init

# Get dependencies
grpcurl -plaintext -d '{"id": 1}' \
  localhost:9000 provider.ProviderService.GetDependencies

# Evaluate query
grpcurl -plaintext -d '{
  "cap": "referenced",
  "conditionInfo": "{\"referenced\": {\"pattern\": \"java.util.List\", \"location\": \"import\"}}"
}' localhost:9000 provider.ProviderService.Evaluate
```

## Environment Variables

### RUST_LOG

Controls logging level:

```bash
# Default (set in Dockerfile)
RUST_LOG=INFO,java_analyzer_provider=DEBUG

# Override
podman run --rm -e RUST_LOG=DEBUG java-provider:latest
```

### JAVA_HOME

Java installation path:

```bash
# Default (set in Dockerfile)
JAVA_HOME=/usr/lib/jvm/java-17-openjdk

# Override if needed
podman run --rm -e JAVA_HOME=/custom/java java-provider:latest
```

## Volume Mounts

### Recommended Mounts

1. **Source Code** (Read-only):
   ```bash
   -v /path/to/project:/analyzer-lsp/project:ro,Z
   ```

2. **Cache Directory** (Read-write for Maven/Gradle caches):
   ```bash
   -v maven-cache:/home/.m2:Z
   -v gradle-cache:/home/.gradle:Z
   ```

3. **Provider Settings** (Read-only):
   ```bash
   -v $(PWD)/provider_settings.json:/analyzer-lsp/provider_settings.json:ro,Z
   ```

### SELinux Considerations

On SELinux-enabled systems (RHEL, Fedora):
- Use `:z` for shared volumes
- Use `:Z` for exclusive volumes
- Use `:ro` for read-only mounts

## Multi-Stage Build Details

### Stage 1: Builder

```dockerfile
FROM registry.access.redhat.com/ubi9/ubi as builder
RUN dnf install -y rust-toolset
COPY Cargo.* build.rs src/ /java-provider/
RUN cargo build --release
```

**Purpose**: Compile Rust binary with all dependencies

**Cache Optimization**: Mount `/root/.cargo` as cache

**Output**: `/java-provider/target/release/java-analyzer-provider`

### Stage 2: Runtime

```dockerfile
FROM registry.access.redhat.com/ubi9/ubi-minimal
RUN microdnf install -y java-17-openjdk-devel maven
# Install Gradle manually
COPY --from=builder /java-provider/target/release/java-analyzer-provider /usr/local/bin/
```

**Purpose**: Minimal runtime environment

**Size Optimization**: Only copies compiled binary, no source or build tools

## Security Considerations

### Non-Root User

Container runs as UID 1001:
```dockerfile
USER 1001
```

**Why**: OpenShift security best practice, prevents privilege escalation

### Read-Only Root Filesystem (Future)

Current: Read-write root filesystem (required for Maven/Gradle caches)

Future enhancement:
```bash
podman run --read-only \
  -v temp:/tmp:Z \
  -v maven-cache:/home/.m2:Z \
  java-provider:latest
```

### Network Policies

Provider only needs outbound access to:
- Maven Central (for dependency resolution)
- Gradle Plugin Portal (for dependency resolution)

No inbound access required beyond gRPC port.

## Troubleshooting

### Image Won't Build

**Issue**: Rust compilation fails

**Solution**: Clear cache and rebuild
```bash
podman build --no-cache -f Dockerfile -t java-provider:latest .
```

### Container Exits Immediately

**Issue**: Binary crashes on startup

**Solution**: Check logs
```bash
podman logs <container-id>
```

**Common Causes**:
- Port already in use
- Missing environment variables
- Invalid command-line arguments

### Permission Denied on Volume Mounts

**Issue**: SELinux blocking access

**Solution**: Add `:Z` label
```bash
-v /path:/mount:Z
```

### Maven/Gradle Not Working

**Issue**: Tools not found in PATH

**Solution**: Verify installation
```bash
podman run --rm -it --entrypoint /bin/bash java-provider:latest
# Inside container:
which java   # Should be /usr/bin/java
which mvn    # Should be /usr/bin/mvn
which gradle # Should be /usr/local/bin/gradle
```

## Performance Optimization

### Build Cache

Use BuildKit cache mounts:
```dockerfile
RUN --mount=type=cache,target=/root/.cargo cargo build --release
```

### Layer Ordering

Dependencies before source code:
```dockerfile
COPY Cargo.lock Cargo.toml /java-provider/
RUN cargo fetch
COPY src/ /java-provider/src/
RUN cargo build --release
```

### Multi-Arch Builds (Future)

Build for ARM64 and AMD64:
```bash
podman buildx build --platform linux/amd64,linux/arm64 \
  -t java-provider:latest .
```

## CI/CD Integration

### GitHub Actions Example

```yaml
name: Build Docker Image

on:
  push:
    branches: [main]

jobs:
  docker:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      - name: Build image
        run: make build-image
      - name: Test image
        run: |
          make run-container &
          sleep 5
          make run-grpc-capabilities
```

### Container Registry Push

```bash
# Tag for registry
podman tag java-provider:latest quay.io/konveyor/java-provider:latest

# Login
podman login quay.io

# Push
podman push quay.io/konveyor/java-provider:latest
```

## Comparison: Java vs C# Provider Images

| Aspect | Java Provider | C# Provider |
|--------|---------------|-------------|
| Base Runtime | OpenJDK 17 | .NET SDK 9.0 |
| Build Tools | Maven, Gradle | Paket, ilspycmd |
| Image Size | ~450 MB | ~500 MB |
| Build Time | ~5-10 min | ~8-12 min |
| Port | 9000 (default) | 14651 (default) |
| Binary Size | ~15 MB | ~12 MB |

## Future Enhancements

1. **Read-only root filesystem**: Move caches to mounted volumes
2. **Multi-arch builds**: Support ARM64 for M1/M2 Macs
3. **Distroless images**: Further reduce attack surface
4. **Health checks**: Add gRPC health check endpoint
5. **Metrics**: Prometheus metrics endpoint
6. **Init system**: Add tini for proper signal handling

## References

- [UBI Images](https://catalog.redhat.com/software/containers/ubi9/ubi/615bcf606feffc5384e8452e)
- [Podman Documentation](https://docs.podman.io/)
- [OpenShift Container Guidelines](https://docs.openshift.com/container-platform/4.13/openshift_images/create-images.html)
- [Konveyor Analyzer LSP](https://github.com/konveyor/analyzer-lsp)
