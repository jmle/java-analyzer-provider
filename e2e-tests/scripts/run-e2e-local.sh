#!/bin/bash
set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
E2E_DIR="$(dirname "${SCRIPT_DIR}")"
PROVIDER_PORT=${PROVIDER_PORT:-9000}
RULESET=${RULESET:-comprehensive}
OUTPUT_FILE="${E2E_DIR}/testdata/${RULESET}-output.yaml"

# Ensure testdata directory exists
mkdir -p "${E2E_DIR}/testdata"

# Check konveyor-analyzer exists
if [ ! -f "${E2E_DIR}/konveyor-analyzer" ]; then
    echo "ERROR: konveyor-analyzer not found. Run 'make e2e-setup' first."
    exit 1
fi

# Wait for provider
echo "Waiting for provider on localhost:${PROVIDER_PORT}..."
for i in {1..30}; do
    if nc -z localhost ${PROVIDER_PORT} 2>/dev/null; then
        echo "✓ Provider is ready!"
        break
    fi
    if [ $i -eq 30 ]; then
        echo "ERROR: Provider did not start in time"
        exit 1
    fi
    sleep 1
done

# Run analyzer
echo "Running konveyor-analyzer with ${RULESET} rules..."
"${E2E_DIR}/konveyor-analyzer" \
    --provider-settings="${E2E_DIR}/provider_settings.json" \
    --rules="${E2E_DIR}/rules/${RULESET}.yaml" \
    --output-file="${OUTPUT_FILE}" \
    --verbose=5

echo "✓ E2E test complete. Output: ${OUTPUT_FILE}"
