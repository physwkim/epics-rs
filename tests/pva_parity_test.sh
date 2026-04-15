#!/bin/bash
# PVA tool parity test: compare C++ pvxs output vs Rust pvXXX-rs output
#
# Prerequisites:
#   - C++ pvxs tools in PATH (pvget, pvinfo, pvput)
#   - Rust tools built: cargo build --release -p epics-pva-rs
#   - An IOC running with the test PVs
#
# Usage:
#   ./tests/pva_parity_test.sh

set -euo pipefail

CPP_PATH="$HOME/epics/epics-base/bin/darwin-aarch64"
RS_PATH="$(dirname "$0")/../target/release"

export EPICS_PVA_BROADCAST_PORT=${EPICS_PVA_BROADCAST_PORT:-5076}

PASS=0
FAIL=0
SKIP=0

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[0;33m'
NC='\033[0m'

compare() {
    local label="$1"
    local cpp_out="$2"
    local rs_out="$3"

    # Normalize: collapse runs of spaces, trim trailing, fuzzy timestamp (±1ms)
    local cpp_norm rs_norm
    cpp_norm=$(echo "$cpp_out" | sed 's/[[:space:]]*$//' | sed 's/  */ /g')
    rs_norm=$(echo "$rs_out" | sed 's/[[:space:]]*$//' | sed 's/  */ /g')
    # Replace timestamps with a stable marker for comparison
    # Pattern: YYYY-MM-DD HH:MM:SS.mmm → YYYY-MM-DD HH:MM:SS.xxx
    cpp_norm=$(echo "$cpp_norm" | sed -E 's/([0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2})\.[0-9]{3}/\1.xxx/g')
    rs_norm=$(echo "$rs_norm" | sed -E 's/([0-9]{4}-[0-9]{2}-[0-9]{2} [0-9]{2}:[0-9]{2}:[0-9]{2})\.[0-9]{3}/\1.xxx/g')

    if [ "$cpp_norm" = "$rs_norm" ]; then
        echo -e "  ${GREEN}PASS${NC} $label"
        PASS=$((PASS + 1))
    else
        echo -e "  ${RED}FAIL${NC} $label"
        echo "    --- C++ ---"
        echo "$cpp_out" | head -5 | sed 's/^/    /'
        echo "    --- Rust ---"
        echo "$rs_out" | head -5 | sed 's/^/    /'
        # Show first difference
        diff <(echo "$cpp_norm") <(echo "$rs_norm") | head -10 | sed 's/^/    /'
        FAIL=$((FAIL + 1))
    fi
}

run_test() {
    local label="$1"
    local cpp_cmd="$2"
    local rs_cmd="$3"

    local cpp_out rs_out
    cpp_out=$(eval "$cpp_cmd" 2>&1) || true
    rs_out=$(eval "$rs_cmd" 2>&1) || true

    compare "$label" "$cpp_out" "$rs_out"
}

# ── Test PVs ─────────────────────────────────────────────────────────────

# Scalar types
SCALAR_PVS=(
    "SIM1:cam1:Gain_RBV"           # double
    "SIM1:cam1:AcquireTime_RBV"    # double with timestamp
    "SIM1:cam1:SizeX_RBV"          # int
    "SIM1:cam1:ArrayCounter_RBV"   # int with timestamp
)

# Enum types
ENUM_PVS=(
    "SIM1:cam1:ImageMode_RBV"      # NTEnum
    "SIM1:cam1:DetectorState_RBV"   # NTEnum
    "SIM1:cam1:DataType_RBV"        # NTEnum
)

echo "========================================"
echo " PVA Tool Parity Test: C++ vs Rust"
echo "========================================"
echo ""

# ── pvget: NT mode (default) ────────────────────────────────────────────

echo "pvget (NT mode):"
for pv in "${SCALAR_PVS[@]}" "${ENUM_PVS[@]}"; do
    run_test "$pv" \
        "$CPP_PATH/pvget -w 2 '$pv'" \
        "$RS_PATH/pvget-rs '$pv'"
done
echo ""

# ── pvget: raw/verbose mode (-v) ────────────────────────────────────────

echo "pvget -v (raw mode):"
for pv in "${SCALAR_PVS[@]}"; do
    run_test "$pv" \
        "$CPP_PATH/pvget -w 2 -v '$pv'" \
        "$RS_PATH/pvget-rs -v '$pv'"
done
for pv in "${ENUM_PVS[@]}"; do
    run_test "$pv" \
        "$CPP_PATH/pvget -w 2 -v '$pv'" \
        "$RS_PATH/pvget-rs -v '$pv'"
done
echo ""

# ── pvget: JSON mode ────────────────────────────────────────────────────

echo "pvget -M json:"
for pv in "${SCALAR_PVS[@]}" "${ENUM_PVS[@]}"; do
    run_test "$pv" \
        "$CPP_PATH/pvget -w 2 -M json '$pv'" \
        "$RS_PATH/pvget-rs -M json '$pv'"
done
echo ""

# ── pvinfo ──────────────────────────────────────────────────────────────

echo "pvinfo:"
for pv in "${SCALAR_PVS[@]}" "${ENUM_PVS[@]}"; do
    run_test "$pv" \
        "$CPP_PATH/pvinfo -w 2 '$pv'" \
        "$RS_PATH/pvinfo-rs '$pv'"
done
# NTNDArray
run_test "SIM1:Pva1:Image" \
    "$CPP_PATH/pvinfo -w 2 'SIM1:Pva1:Image'" \
    "$RS_PATH/pvinfo-rs 'SIM1:Pva1:Image'"
echo ""

# ── pvget: NTNDArray (compare structure only, skip huge array data) ─────

echo "pvget -v NTNDArray (structure tail):"
cpp_ndarray=$($CPP_PATH/pvget -w 2 -v SIM1:Pva1:Image 2>&1 | tail -30) || true
rs_ndarray=$($RS_PATH/pvget-rs -v SIM1:Pva1:Image 2>&1 | tail -30) || true
compare "SIM1:Pva1:Image (tail-30)" "$cpp_ndarray" "$rs_ndarray"
echo ""

# ── Summary ─────────────────────────────────────────────────────────────

echo "========================================"
TOTAL=$((PASS + FAIL + SKIP))
echo -e " Results: ${GREEN}$PASS passed${NC}, ${RED}$FAIL failed${NC}, ${YELLOW}$SKIP skipped${NC} / $TOTAL total"
echo "========================================"

if [ "$FAIL" -gt 0 ]; then
    exit 1
fi
