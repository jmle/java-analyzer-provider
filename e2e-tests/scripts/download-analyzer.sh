#!/bin/bash
set -e

KONVEYOR_BRANCH=${KONVEYOR_BRANCH:-main}
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
E2E_DIR="$(dirname "${SCRIPT_DIR}")"
DEST="${E2E_DIR}/konveyor-analyzer"

if [ -f "$DEST" ]; then
    echo "konveyor-analyzer already exists at $DEST"
    exit 0
fi

echo "Downloading and building konveyor-analyzer from branch ${KONVEYOR_BRANCH}..."
TEMP_DIR=$(mktemp -d)
trap "rm -rf ${TEMP_DIR}" EXIT

git clone --depth 1 --branch ${KONVEYOR_BRANCH} \
    https://github.com/konveyor/analyzer-lsp.git "${TEMP_DIR}"

cd "${TEMP_DIR}"
echo "Building konveyor-analyzer..."
make build

cp build/konveyor-analyzer "${DEST}"
chmod +x "${DEST}"

echo "✓ konveyor-analyzer ready at ${DEST}"
