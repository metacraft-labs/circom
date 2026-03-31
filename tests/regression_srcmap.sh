#!/usr/bin/env bash
# Regression test: verify that --srcmap has zero impact on existing outputs.
#
# Compiles test circuits with and without --srcmap and diffs the non-srcmap
# outputs (wasm, r1cs, sym) to prove the flag is purely additive.
set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "$0")" && pwd)"
CIRCOM="${CIRCOM:-circom}"
TEST_CIRCUITS_DIR="${TEST_CIRCUITS_DIR:-$SCRIPT_DIR/../../codetracer-circom-recorder/test-programs/circom}"
TMPDIR="$(mktemp -d)"

trap 'rm -rf "$TMPDIR"' EXIT

passed=0
failed=0

run_test() {
  local circuit="$1"
  local name
  name="$(basename "$circuit" .circom)"

  echo "Testing: $name"

  local dir_without="$TMPDIR/${name}_without"
  local dir_with="$TMPDIR/${name}_with"
  mkdir -p "$dir_without" "$dir_with"

  # Compile WITHOUT --srcmap
  "$CIRCOM" "$circuit" --wasm --r1cs --sym -o "$dir_without" 2>/dev/null

  # Compile WITH --srcmap
  "$CIRCOM" "$circuit" --wasm --r1cs --sym --srcmap -o "$dir_with" 2>/dev/null

  # Verify .srcmap.json was generated
  if [ ! -f "$dir_with/${name}.srcmap.json" ]; then
    echo "  FAIL: .srcmap.json not generated"
    failed=$((failed + 1))
    return
  fi
  echo "  OK: .srcmap.json generated"

  # Verify .srcmap.json is valid JSON
  if ! python3 -m json.tool "$dir_with/${name}.srcmap.json" >/dev/null 2>&1; then
    echo "  FAIL: .srcmap.json is not valid JSON"
    failed=$((failed + 1))
    return
  fi
  echo "  OK: .srcmap.json is valid JSON"

  # Compare .r1cs files (binary)
  if ! cmp -s "$dir_without/${name}.r1cs" "$dir_with/${name}.r1cs"; then
    echo "  FAIL: .r1cs differs with --srcmap"
    failed=$((failed + 1))
    return
  fi
  echo "  OK: .r1cs identical"

  # Compare .sym files
  if ! cmp -s "$dir_without/${name}.sym" "$dir_with/${name}.sym"; then
    echo "  FAIL: .sym differs with --srcmap"
    failed=$((failed + 1))
    return
  fi
  echo "  OK: .sym identical"

  # Compare wasm directories (all generated files)
  local wasm_diff
  wasm_diff="$(diff -rq "$dir_without/${name}_js" "$dir_with/${name}_js" 2>/dev/null || true)"
  if [ -n "$wasm_diff" ]; then
    echo "  FAIL: wasm output differs with --srcmap"
    echo "  $wasm_diff"
    failed=$((failed + 1))
    return
  fi
  echo "  OK: wasm output identical"

  # Validate srcmap content structure
  local version
  version="$(python3 -c "import json; d=json.load(open('$dir_with/${name}.srcmap.json')); print(d.get('version', 'MISSING'))")"
  if [ "$version" != "1" ]; then
    echo "  FAIL: srcmap version is '$version', expected '1'"
    failed=$((failed + 1))
    return
  fi

  local mapping_count
  mapping_count="$(python3 -c "import json; d=json.load(open('$dir_with/${name}.srcmap.json')); print(len(d.get('mappings', [])))")"
  if [ "$mapping_count" -eq 0 ]; then
    echo "  FAIL: srcmap has 0 mappings"
    failed=$((failed + 1))
    return
  fi
  echo "  OK: srcmap has $mapping_count mappings"

  passed=$((passed + 1))
  echo "  PASS"
}

echo "=== Circom --srcmap Regression Test ==="
echo "Circom binary: $CIRCOM"
echo "Test circuits: $TEST_CIRCUITS_DIR"
echo ""

for circuit in "$TEST_CIRCUITS_DIR"/*.circom; do
  [ -f "$circuit" ] || continue
  run_test "$circuit"
  echo ""
done

echo "=== Results: $passed passed, $failed failed ==="
if [ "$failed" -gt 0 ]; then
  exit 1
fi
