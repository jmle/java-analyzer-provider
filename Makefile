# Container runtime (podman by default, can be overridden with docker)
CONTAINER_RUNTIME ?= podman

# Branch to download konveyor-analyzer from (defaults to main)
KONVEYOR_BRANCH ?= main

# SELinux label for shared volumes (use :z for shared, :Z for exclusive)
MOUNT_OPT ?= :U,z

TAG ?= latest
IMAGE ?= java-provider:${TAG}
IMG_ANALYZER ?= quay.io/konveyor/analyzer-lsp:$(TAG)

.PHONY: all clean test download_proto build run build-image test-docker run-tests e2e-setup e2e-local e2e-verify e2e-generate-expected e2e-clean e2e

all: build

clean:
	cargo clean
	rm -f e2e-tests/konveyor-analyzer
	rm -f e2e-tests/analysis-output.yaml

test: run-tests

download_proto:
	curl -L -o src/build/proto/provider.proto https://raw.githubusercontent.com/konveyor/analyzer-lsp/refs/heads/main/provider/internal/grpc/library.proto

build:
	cargo build

build-release:
	cargo build --release

run:
	cargo run -- 9000

build-image:
	$(CONTAINER_RUNTIME) build -f Dockerfile -t ${IMAGE} .

build-test-image:
	$(CONTAINER_RUNTIME) build -f Dockerfile.test -t ${IMAGE}-test .

test-docker: build-test-image
	$(CONTAINER_RUNTIME) run --rm ${IMAGE}-test

run-tests:
	cargo test -- --nocapture

run-tests-quiet:
	cargo test

# Local gRPC testing
wait-for-server:
	@echo "Waiting for server to start listening on localhost:9000..."
	@for i in $$(seq 1 300); do \
		if nc -z localhost 9000; then \
			echo "Server is listening!"; \
			exit 0; \
		else \
			echo "Attempt $$i: Server not ready. Waiting 1s..."; \
			sleep 1; \
		fi; \
	done

# gRPC testing with grpcurl (requires running server)
run-grpc-init:
	grpcurl -plaintext -d '{"location": "$(PWD)/tests/fixtures", "analysisMode": "source-only"}' localhost:9000 provider.ProviderService.Init

run-grpc-capabilities:
	grpcurl -plaintext localhost:9000 provider.ProviderService.GetCapabilities

run-grpc-dependencies:
	grpcurl -plaintext -d '{"id": 1}' localhost:9000 provider.ProviderService.GetDependencies

run-grpc-evaluate:
	grpcurl -max-msg-sz 10485760 -plaintext -d '{"cap": "referenced", "conditionInfo": "{\"referenced\": {\"pattern\": \"java.util.List\", \"location\": \"import\"}}"}' localhost:9000 provider.ProviderService.Evaluate

# Format and lint
fmt:
	cargo fmt

lint:
	cargo clippy --all-targets --all-features

# Build and run container locally
run-container: build-image
	$(CONTAINER_RUNTIME) run --rm -p 9000:9000 ${IMAGE}

# Run container with test data mounted
run-container-with-data: build-image
	$(CONTAINER_RUNTIME) run --rm -p 9000:9000 \
		-v $(PWD)/tests/fixtures:/analyzer-lsp/test-data$(MOUNT_OPT) \
		${IMAGE}

# Pod-based testing (similar to C# provider)
run-java-pod:
	$(CONTAINER_RUNTIME) volume create test-data
	$(CONTAINER_RUNTIME) run --rm -v test-data:/target$(MOUNT_OPT) -v $(PWD)/tests/fixtures:/src/$(MOUNT_OPT) --entrypoint=cp alpine -a /src/. /target/
	$(CONTAINER_RUNTIME) pod create --name=analyzer-java
	$(CONTAINER_RUNTIME) run --pod analyzer-java --name java-provider -d -v test-data:/analyzer-lsp/examples$(MOUNT_OPT) ${IMAGE} 14652

stop-java-pod:
	$(CONTAINER_RUNTIME) pod kill analyzer-java || true
	$(CONTAINER_RUNTIME) pod rm analyzer-java || true
	$(CONTAINER_RUNTIME) volume rm test-data || true

# E2E Testing
e2e-setup:
	@if [ ! -f e2e-tests/konveyor-analyzer ]; then \
		echo "Downloading konveyor-analyzer..."; \
		./e2e-tests/scripts/download-analyzer.sh; \
	else \
		echo "✓ konveyor-analyzer already downloaded"; \
	fi

e2e-local: build e2e-setup
	@echo "Starting provider on port 9000..."
	@./target/debug/java-analyzer-provider 9000 > e2e-tests/provider.log 2>&1 & \
	echo $$! > e2e-tests/provider.pid
	@sleep 3
	@echo "Running E2E tests..."
	@cd e2e-tests && PROVIDER_PORT=9000 ./scripts/run-e2e-local.sh || (kill `cat provider.pid` 2>/dev/null; rm -f provider.pid; exit 1)
	@kill `cat e2e-tests/provider.pid` 2>/dev/null || true
	@rm -f e2e-tests/provider.pid

e2e-verify:
	@./e2e-tests/scripts/verify-output.sh \
		e2e-tests/testdata/comprehensive-output.yaml \
		e2e-tests/expected/comprehensive-output.yaml

e2e-generate-expected:
	@mkdir -p e2e-tests/expected
	@if [ -f e2e-tests/testdata/comprehensive-output.yaml ]; then \
		cp e2e-tests/testdata/comprehensive-output.yaml e2e-tests/expected/comprehensive-output.yaml; \
		echo "✓ Baseline created at e2e-tests/expected/comprehensive-output.yaml"; \
	else \
		echo "ERROR: Run 'make e2e-local' first to generate output"; \
		exit 1; \
	fi

e2e-clean:
	rm -f e2e-tests/konveyor-analyzer
	rm -rf e2e-tests/testdata
	rm -f e2e-tests/provider.pid
	rm -f e2e-tests/provider.log

e2e: e2e-local e2e-verify

# Help target
help:
	@echo "Java Analyzer Provider - Available targets:"
	@echo ""
	@echo "Building:"
	@echo "  make build              - Build the project in debug mode"
	@echo "  make build-release      - Build the project in release mode"
	@echo "  make build-image        - Build Docker image"
	@echo "  make build-test-image   - Build test Docker image"
	@echo ""
	@echo "Testing:"
	@echo "  make test               - Run all tests"
	@echo "  make run-tests          - Run tests with output"
	@echo "  make run-tests-quiet    - Run tests without output"
	@echo "  make test-docker        - Run tests in Docker container"
	@echo ""
	@echo "Running:"
	@echo "  make run                - Run the provider locally on port 9000"
	@echo "  make run-container      - Run the provider in a container"
	@echo "  make run-java-pod       - Run provider in a pod with test data"
	@echo "  make stop-java-pod      - Stop and cleanup pod"
	@echo ""
	@echo "gRPC Testing (requires running server):"
	@echo "  make wait-for-server    - Wait for server to be ready"
	@echo "  make run-grpc-init      - Test Init RPC"
	@echo "  make run-grpc-capabilities - Test GetCapabilities RPC"
	@echo "  make run-grpc-dependencies - Test GetDependencies RPC"
	@echo "  make run-grpc-evaluate  - Test Evaluate RPC"
	@echo ""
	@echo "E2E Testing:"
	@echo "  make e2e-setup          - Download konveyor-analyzer binary"
	@echo "  make e2e-local          - Run E2E tests locally"
	@echo "  make e2e-verify         - Verify output against baseline"
	@echo "  make e2e-generate-expected - Generate expected baseline from last run"
	@echo "  make e2e-clean          - Clean E2E test artifacts"
	@echo "  make e2e                - Run E2E tests and verify (e2e-local + e2e-verify)"
	@echo ""
	@echo "Code Quality:"
	@echo "  make fmt                - Format code with rustfmt"
	@echo "  make lint               - Lint code with clippy"
	@echo ""
	@echo "Utilities:"
	@echo "  make clean              - Clean build artifacts"
	@echo "  make download_proto     - Download latest proto file"
	@echo "  make help               - Show this help message"
