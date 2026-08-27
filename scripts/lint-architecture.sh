#!/usr/bin/env bash
# scripts/lint-architecture.sh
# Mechanical enforcement of architecture rules from AGENTS.md / rule.md
# Run: bash scripts/lint-architecture.sh
# CI:  Add to GitHub Actions as a step

set -euo pipefail

BACKEND_SRC="backend/src"
ERRORS=0

echo "=== IronixPay Architecture Lint ==="
echo ""

# ─────────────────────────────────────────────
# Rule 1: No println! in backend code
# ─────────────────────────────────────────────
echo "▶ [Rule 1] Checking for println! usage..."
PRINTLN_HITS=$(grep -rn 'println!' "$BACKEND_SRC" --include='*.rs' \
  | grep -v '/bin/' \
  | grep -v '// allow-println' \
  | grep -v '#\[cfg(test)\]' \
  | grep -v 'mod tests' \
  || true)

if [ -n "$PRINTLN_HITS" ]; then
  echo "  ✗ FAIL: Found println! — use tracing::{info, warn, error, debug} instead"
  echo "$PRINTLN_HITS" | head -10
  ERRORS=$((ERRORS + 1))
else
  echo "  ✓ OK"
fi

# ─────────────────────────────────────────────
# Rule 2: Entity files must have created_at + updated_at
# (Warning only — some entities are append-only by design)
# ─────────────────────────────────────────────
echo "▶ [Rule 2] Checking entity timestamp fields..."
# Append-only entities that legitimately lack updated_at
TIMESTAMP_ALLOWLIST="billing_logs.rs|idempotency_keys.rs|indexer_state.rs|payout_trusted_addresses.rs"

for entity_file in "$BACKEND_SRC"/entity/*.rs; do
  basename=$(basename "$entity_file")
  # Skip mod.rs, prelude.rs, and sea_orm_active_enums.rs
  if [[ "$basename" == "mod.rs" || "$basename" == "prelude.rs" || "$basename" == "sea_orm_active_enums.rs" ]]; then
    continue
  fi

  # Skip allowlisted entities
  if echo "$basename" | grep -qE "$TIMESTAMP_ALLOWLIST"; then
    continue
  fi

  # Check if file defines a Model struct (actual entity file)
  if ! grep -q 'DeriveEntityModel' "$entity_file"; then
    continue
  fi

  missing=""
  if ! grep -q 'created_at' "$entity_file"; then
    missing="created_at"
  fi
  if ! grep -q 'updated_at' "$entity_file"; then
    missing="${missing:+$missing, }updated_at"
  fi

  if [ -n "$missing" ]; then
    echo "  ⚠ WARNING: $basename is missing: $missing"
    # Warning only, don't increment ERRORS
  fi
done
echo "  ✓ Entity timestamp check complete"

# ─────────────────────────────────────────────
# Rule 3: File size limit (800 lines)
# ─────────────────────────────────────────────
MAX_LINES=800
echo "▶ [Rule 3] Checking for files exceeding $MAX_LINES lines..."
LARGE_FILES=$(find "$BACKEND_SRC" -name '*.rs' -exec wc -l {} + \
  | awk -v max="$MAX_LINES" '$1 > max && !/total$/' \
  | sort -rn || true)

if [ -n "$LARGE_FILES" ]; then
  echo "  ⚠ WARNING: Files exceeding $MAX_LINES lines (consider splitting):"
  echo "$LARGE_FILES" | head -10
  # Warning only, don't fail the build
else
  echo "  ✓ OK"
fi

# ─────────────────────────────────────────────
# Rule 4: No .env files committed
# ─────────────────────────────────────────────
echo "▶ [Rule 4] Checking for committed .env files..."
ENV_FILES=$(find . -name '.env' -not -path './.git/*' -not -path '*/node_modules/*' -not -path './examples/*' -not -name '*.template' 2>/dev/null || true)
if [ -n "$ENV_FILES" ]; then
  echo "  ✗ FAIL: Found .env files (must not be committed):"
  echo "$ENV_FILES"
  ERRORS=$((ERRORS + 1))
else
  echo "  ✓ OK"
fi

# ─────────────────────────────────────────────
# Summary
# ─────────────────────────────────────────────
echo ""
if [ "$ERRORS" -gt 0 ]; then
  echo "=== FAILED: $ERRORS rule violation(s) ==="
  exit 1
else
  echo "=== PASSED: All architecture rules OK ==="
  exit 0
fi
