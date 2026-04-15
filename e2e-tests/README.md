# End-to-End Testing for Java Analyzer Provider

## Overview

This directory contains end-to-end (E2E) tests that verify the Rust-based Java analyzer provider works correctly with the actual `konveyor-analyzer` binary from analyzer-lsp. These tests ensure the provider can analyze real Java projects and produce correct rule violations.

## What E2E Tests Cover

1. **Real Projects**: Test against actual Maven/Gradle projects (not just toy fixtures)
2. **Full Pipeline**: gRPC → Parser → TypeResolver → Query Engine → Rule Evaluation → Output
3. **All Capabilities**: 15 location types + dependency analysis + pattern matching
4. **Integration**: Verify the provider works with the actual konveyor-analyzer binary
5. **Regression**: Establish baseline outputs for future verification

## Directory Structure

```
e2e-tests/
├── README.md                          # This file
├── provider_settings.json             # Provider configuration
├── rules/                             # Test rulesets
│   ├── comprehensive.yaml             # All 47 rules from original Go provider
│   └── location-types.yaml            # One rule per location type (15 rules)
├── examples/                          # Sample Java projects
│   ├── java/example/                  # Basic Maven with Kubernetes API
│   ├── customers-tomcat-legacy/       # Spring/Tomcat legacy app
│   ├── gradle-multi-project-example/  # Multi-module Gradle
│   ├── inclusion-tests/               # Path filtering tests
│   └── sample-tiles-app/              # Apache Tiles Spring app
├── expected/                          # Baseline outputs for regression
│   └── comprehensive-output.yaml      # Expected output (created after first run)
├── scripts/                           # Helper scripts
│   ├── download-analyzer.sh           # Download konveyor-analyzer
│   ├── run-e2e-local.sh               # Run E2E tests locally
│   └── verify-output.sh               # Verify test output
├── testdata/                          # Actual test outputs (gitignored)
│   └── comprehensive-output.yaml      # Output from last run
├── konveyor-analyzer                  # Downloaded analyzer binary (gitignored)
├── provider.pid                       # Provider process ID (gitignored)
└── provider.log                       # Provider logs (gitignored)
```

## Test Projects

### 1. java/example
- **Type**: Maven single-module project
- **Purpose**: Tests basic Java parsing with Kubernetes API references
- **Key Features**: TYPE, IMPORT, METHOD_CALL locations

### 2. customers-tomcat-legacy
- **Type**: Maven multi-module Spring/Tomcat app
- **Purpose**: Tests legacy Spring application analysis
- **Key Features**: Annotations, Spring beans, JPA repositories

### 3. gradle-multi-project-example
- **Type**: Gradle multi-module project
- **Purpose**: Tests Gradle dependency resolution
- **Key Features**: Multi-module structure, Gradle DSL

### 4. inclusion-tests
- **Type**: Maven project with path filtering
- **Purpose**: Tests `includedPaths` provider configuration
- **Key Features**: Path-based filtering

### 5. sample-tiles-app
- **Type**: Maven Spring app with Apache Tiles
- **Purpose**: Tests complex Spring configuration
- **Key Features**: PACKAGE, FIELD, METHOD, annotation inspection

## Running E2E Tests

### Prerequisites

- Rust 1.75+ (for building the provider)
- Java 17+ (for sample projects)
- Maven 3.6+ (for Maven projects)
- Gradle 8+ (for Gradle projects)
- Go 1.19+ (for building konveyor-analyzer)
- netcat (for checking port availability)

### Quick Start

```bash
# From project root
make e2e
```

This will:
1. Build the provider (`cargo build`)
2. Download konveyor-analyzer if needed
3. Start the provider on port 9000
4. Run E2E tests with comprehensive rules
5. Verify output against baseline
6. Stop the provider

### Step-by-Step

#### 1. Download konveyor-analyzer

```bash
make e2e-setup
```

This downloads and builds the konveyor-analyzer binary from the main branch.

**Custom Branch**:
```bash
KONVEYOR_BRANCH=release-0.5 make e2e-setup
```

#### 2. Run E2E Tests

```bash
make e2e-local
```

This:
- Builds the provider
- Starts it on port 9000
- Runs analyzer with comprehensive rules
- Saves output to `e2e-tests/testdata/comprehensive-output.yaml`
- Stops the provider

**Custom Ruleset**:
```bash
cd e2e-tests
RULESET=location-types ./scripts/run-e2e-local.sh
```

#### 3. Verify Output

```bash
make e2e-verify
```

This compares the actual output against the expected baseline.

**First Run**: If no baseline exists, the verify script will warn you and exit successfully.

#### 4. Generate Baseline

After verifying the output is correct (manually inspect `e2e-tests/testdata/comprehensive-output.yaml`):

```bash
make e2e-generate-expected
```

This creates the baseline at `e2e-tests/expected/comprehensive-output.yaml`.

**Commit the baseline** to track expected behavior.

### Manual Testing

If you need more control:

```bash
# 1. Build provider
cargo build

# 2. Download analyzer
./e2e-tests/scripts/download-analyzer.sh

# 3. Start provider in background
./target/debug/java-analyzer-provider 9000 &
PROVIDER_PID=$!

# 4. Wait for startup
sleep 3

# 5. Run tests
cd e2e-tests
./scripts/run-e2e-local.sh

# 6. Stop provider
kill $PROVIDER_PID
```

## Understanding Test Output

### Output Format

The output YAML has this structure:

```yaml
rulesets:
  - name: comprehensive
    violations:
      lang-ref-003:
        - uri: file:///analyzer-lsp/examples/java/example/src/main/java/com/example/apps/App.java
          lineNumber: 5
          message: "java found apiextensions/v1/customresourcedefinitions found ..."
          codeSnip: "import io.fabric8.kubernetes.api.model.apiextensions.v1beta1.CustomResourceDefinition;"
      java-pomxml-dependencies:
        - uri: file:///analyzer-lsp/examples/java/example/pom.xml
          lineNumber: 1
          message: "dependency junit.junit with 4.12 is bad ..."
      # ... more rules
```

### Verification Process

The `verify-output.sh` script checks:

1. **Rule Count**: Are all expected rules matched?
2. **Incident Count**: Are the number of violations within tolerance (±10%)?

**Why Tolerance**: Different Rust/Go parsing may produce slightly different line numbers or match counts. A 10% variance is acceptable.

### Expected Results

For the comprehensive ruleset, expect:
- **~40-50 rules matched** (out of 47 rules in comprehensive.yaml)
- **~500-2000+ incidents** (depends on sample projects)

Not all rules will match every time:
- Some rules target specific files/features
- Some projects may not have the pattern

## Troubleshooting

### Provider Won't Start

**Symptom**: `ERROR: Provider did not start in time`

**Solutions**:
```bash
# Check port is not in use
lsof -i :9000

# Check provider logs
cat e2e-tests/provider.log

# Try running provider manually
cargo run -- 9000
```

### konveyor-analyzer Not Found

**Symptom**: `ERROR: konveyor-analyzer not found`

**Solution**:
```bash
make e2e-setup
# or
./e2e-tests/scripts/download-analyzer.sh
```

### Build Failures

**Symptom**: konveyor-analyzer build fails

**Solution**:
```bash
# Try specific branch
KONVEYOR_BRANCH=release-0.5 make e2e-setup

# Or download pre-built binary
# (requires konveyor to publish releases)
```

### Verification Failures

**Symptom**: `ERROR: Rule count mismatch`

**Causes**:
1. Provider implementation changed
2. Sample projects changed
3. Rules changed

**Solution**:
```bash
# Inspect actual output
cat e2e-tests/testdata/comprehensive-output.yaml

# Compare with expected
diff e2e-tests/testdata/comprehensive-output.yaml \
     e2e-tests/expected/comprehensive-output.yaml

# If changes are intentional, regenerate baseline
make e2e-generate-expected
```

### Path Issues in Container

**Symptom**: Provider can't find sample projects

**Solution**: The `provider_settings.json` uses paths like `/analyzer-lsp/examples/*`. These work both locally (relative symlink) and in containers.

For local testing, paths are relative to project root:
```json
{
  "location": "/analyzer-lsp/examples/java/example"
}
```

Resolves to: `<project-root>/e2e-tests/examples/java/example`

## Advanced Usage

### Testing Specific Rules

Create a custom ruleset:

```bash
cat > e2e-tests/rules/custom.yaml << 'EOF'
- ruleID: test-imports
  when:
    java.referenced:
      pattern: java.util.List
      location: IMPORT
  message: "Found List import"
EOF

cd e2e-tests
RULESET=custom ./scripts/run-e2e-local.sh
```

### Testing with Different Projects

Modify `provider_settings.json`:

```json
[
  {
    "name": "java",
    "address": "localhost:9000",
    "initConfig": [
      {
        "location": "/path/to/your/project",
        "analysisMode": "source-only"
      }
    ]
  }
]
```

### Performance Testing

```bash
# Time E2E run
time make e2e-local

# Check provider logs for timing
grep "took" e2e-tests/provider.log
```

### Debugging Provider

```bash
# Run provider with debug logging
RUST_LOG=DEBUG cargo run -- 9000 &

# Or attach debugger
rust-gdb --args target/debug/java-analyzer-provider 9000
```

## Differences from Go Provider

### Expected Differences

1. **Line Numbers**: May vary slightly due to different parsing
2. **Match Counts**: Rust provider may find more/fewer matches
3. **Code Snippets**: Format may differ slightly
4. **Performance**: Rust provider is typically faster

### Not Yet Implemented

- Binary artifact analysis (`mvn://` URIs)
- Source + dependencies mode (combined analysis)

### Simplified vs Go Provider

- No JDTLS configuration needed
- No separate Java process
- Simpler provider_settings.json

## CI/CD Integration

### GitHub Actions Example

```yaml
name: E2E Tests

on: [push, pull_request]

jobs:
  e2e:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v3
      
      - name: Setup Rust
        uses: actions-rs/toolchain@v1
        with:
          toolchain: stable
      
      - name: Setup Go
        uses: actions/setup-go@v4
        with:
          go-version: '1.21'
      
      - name: Run E2E tests
        run: make e2e
      
      - name: Upload output
        if: failure()
        uses: actions/upload-artifact@v3
        with:
          name: e2e-output
          path: e2e-tests/testdata/
```

## Maintenance

### Updating Sample Projects

```bash
# Copy new projects to e2e-tests/examples/
cp -r /path/to/new-project e2e-tests/examples/

# Update provider_settings.json
# Add new location entry

# Run tests
make e2e-local

# Generate new baseline
make e2e-generate-expected
```

### Updating Rules

```bash
# Edit e2e-tests/rules/comprehensive.yaml
# Add/modify/remove rules

# Run tests
make e2e-local

# Inspect output
cat e2e-tests/testdata/comprehensive-output.yaml

# Update baseline if correct
make e2e-generate-expected
```

### Cleaning Up

```bash
# Remove all E2E artifacts
make e2e-clean

# Force re-download analyzer
rm e2e-tests/konveyor-analyzer
make e2e-setup
```

## Contributing

When adding new features to the provider:

1. **Add test case**: Create or update sample project in `examples/`
2. **Add rule**: Create rule in `comprehensive.yaml` to test the feature
3. **Run E2E**: Verify your feature works end-to-end
4. **Update baseline**: Generate new expected output
5. **Commit**: Include updated baseline in your PR

## References

- [Original Go Provider E2E Tests](https://github.com/konveyor/analyzer-lsp/tree/main/external-providers/java-external-provider/e2e-tests)
- [Konveyor Analyzer LSP](https://github.com/konveyor/analyzer-lsp)
- [Rule Writing Guide](https://konveyor.github.io/konveyor/rules/)
- [Provider Interface](https://github.com/konveyor/analyzer-lsp/blob/main/provider/internal/grpc/library.proto)
