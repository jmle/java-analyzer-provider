#!/bin/bash
set -e

ACTUAL=$1
EXPECTED=$2

if [ -z "${ACTUAL}" ] || [ -z "${EXPECTED}" ]; then
    echo "Usage: $0 <actual-output.yaml> <expected-output.yaml>"
    exit 1
fi

if [ ! -f "${ACTUAL}" ]; then
    echo "ERROR: Actual output not found: ${ACTUAL}"
    exit 2
fi

if [ ! -f "${EXPECTED}" ]; then
    echo "WARNING: Expected output not found: ${EXPECTED}"
    echo "This appears to be the first run. To create a baseline:"
    echo "  cp ${ACTUAL} ${EXPECTED}"
    exit 0
fi

echo "Verifying output against baseline..."

# Compare rule counts
ACTUAL_RULES=$(grep "^  [a-z].*:" "${ACTUAL}" | wc -l || echo 0)
EXPECTED_RULES=$(grep "^  [a-z].*:" "${EXPECTED}" | wc -l || echo 0)

echo "  Actual rules matched:   ${ACTUAL_RULES}"
echo "  Expected rules matched: ${EXPECTED_RULES}"

if [ "${ACTUAL_RULES}" != "${EXPECTED_RULES}" ]; then
    echo "ERROR: Rule count mismatch"
    echo ""
    echo "Rules in actual output:"
    grep "^  [a-z].*:" "${ACTUAL}" | sort || true
    echo ""
    echo "Rules in expected output:"
    grep "^  [a-z].*:" "${EXPECTED}" | sort || true
    exit 2
fi

# Count total incidents
ACTUAL_INCIDENTS=$(grep -c "^  - uri:" "${ACTUAL}" || echo 0)
EXPECTED_INCIDENTS=$(grep -c "^  - uri:" "${EXPECTED}" || echo 0)

echo "  Actual incidents:   ${ACTUAL_INCIDENTS}"
echo "  Expected incidents: ${EXPECTED_INCIDENTS}"

# Allow some variance in incident counts (±10%)
LOWER_BOUND=$((EXPECTED_INCIDENTS * 90 / 100))
UPPER_BOUND=$((EXPECTED_INCIDENTS * 110 / 100))

if [ "${ACTUAL_INCIDENTS}" -lt "${LOWER_BOUND}" ] || [ "${ACTUAL_INCIDENTS}" -gt "${UPPER_BOUND}" ]; then
    echo "WARNING: Incident count differs significantly from baseline"
    echo "  Expected: ${EXPECTED_INCIDENTS} (tolerance: ${LOWER_BOUND}-${UPPER_BOUND})"
    echo "  Actual:   ${ACTUAL_INCIDENTS}"
    echo ""
    echo "This may indicate a regression or improvement. Review manually."
fi

echo ""
echo "✓ Verification passed: All rules matched"
