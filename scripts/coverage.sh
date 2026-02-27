#!/usr/bin/env bash
# scripts/coverage.sh
#
# Generate a test coverage report for the Callora Contracts workspace using
# cargo-tarpaulin and enforce a minimum of 95 % line coverage.
#
# Usage
# -----
#   ./scripts/coverage.sh           # run from the workspace root
#
# First-time setup
# ----------------
#   The script installs cargo-tarpaulin automatically if it is not found.
#   You only need a working Rust / Cargo toolchain (stable).
#
# Output
# ------
#   coverage/tarpaulin-report.html  – interactive per-file report
#   coverage/cobertura.xml          – Cobertura XML (consumed by CI)
#   Stdout summary printed at end of run

set -euo pipefail

# ---------------------------------------------------------------------------
# Configuration — keep in sync with tarpaulin.toml
# ---------------------------------------------------------------------------
MINIMUM_COVERAGE=95
COVERAGE_DIR="coverage"
TARPAULIN_VERSION="0.31"   # minimum version; any newer release also works

# ---------------------------------------------------------------------------
# Helpers
# ---------------------------------------------------------------------------
info()    { echo -e "  \033[1;34m[INFO]\033[0m  $*"; }
success() { echo -e "  \033[1;32m[PASS]\033[0m  $*"; }
error()   { echo -e "  \033[1;31m[FAIL]\033[0m  $*" >&2; }

# ---------------------------------------------------------------------------
# Make sure we run from the workspace root (directory containing Cargo.toml)
# ---------------------------------------------------------------------------
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "${SCRIPT_DIR}/.."

if [[ ! -f "Cargo.toml" ]]; then
    error "Could not locate workspace Cargo.toml. Run this script from the repo root."
    exit 1
fi

# ---------------------------------------------------------------------------
# Install cargo-tarpaulin if missing
# ---------------------------------------------------------------------------
if ! cargo tarpaulin --version &>/dev/null 2>&1; then
    info "cargo-tarpaulin not found — installing (this happens once)..."
    cargo install cargo-tarpaulin --version "^${TARPAULIN_VERSION}" --locked
    success "cargo-tarpaulin installed."
else
    INSTALLED=$(cargo tarpaulin --version 2>&1 | head -1)
    info "Using ${INSTALLED}"
fi

# ---------------------------------------------------------------------------
# Prepare output directory
# ---------------------------------------------------------------------------
mkdir -p "${COVERAGE_DIR}"

# ---------------------------------------------------------------------------
# Run coverage
# tarpaulin.toml in the workspace root carries the full configuration;
# flags below match it so the script can also be run without the config file.
# ---------------------------------------------------------------------------
info "Running tests with coverage instrumentation..."
echo ""

cargo tarpaulin \
    --config tarpaulin.toml

echo ""

# ---------------------------------------------------------------------------
# Friendly reminder of where to find the reports
# ---------------------------------------------------------------------------
success "Coverage run complete."
echo ""
echo "  Reports written to ./${COVERAGE_DIR}/"
echo "    HTML  →  ./${COVERAGE_DIR}/tarpaulin-report.html"
echo "    XML   →  ./${COVERAGE_DIR}/cobertura.xml"
echo ""
echo "  Open the HTML report in a browser:"
echo "    xdg-open ./${COVERAGE_DIR}/tarpaulin-report.html  # Linux"
echo "    open     ./${COVERAGE_DIR}/tarpaulin-report.html  # macOS"
echo ""
echo "  Minimum enforced: ${MINIMUM_COVERAGE}%"
echo "  (non-zero exit from tarpaulin means coverage fell below the threshold)"
