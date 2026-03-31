# Upstream PR: Source Map Generation (--srcmap flag)

**Target:** iden3/circom
**Branch:** codetracer-source-maps
**Scope:** 9 files, +331/-3 lines

## Summary

Add an opt-in `--srcmap` flag that generates a `.srcmap.json` file alongside compiled output, mapping signal operations and declarations back to their source locations in `.circom` files.

## Motivation

Circom's generated witness code (WASM/C++) has no source mapping back to the original `.circom` source. The `.sym` file maps signal names to R1CS positions but does not contain source line information. This makes source-level debugging of circom circuits impossible.

The `--srcmap` flag solves this by walking the compiler's AST (which already tracks `FileLocation` on every node) and emitting a structured JSON source map.

## Usage

```bash
circom circuit.circom --wasm --r1cs --srcmap -o output/
```

This produces `output/circuit.srcmap.json` alongside the usual outputs.

## Source Map Format

```json
{
  "version": 1,
  "files": [
    { "id": 0, "path": "circuit.circom" },
    { "id": 1, "path": "lib/utils.circom" }
  ],
  "mappings": [
    {
      "templateName": "Main",
      "templateId": 0,
      "signalName": "out",
      "statementType": "signal_assign",
      "fileId": 0,
      "sourceFile": "circuit.circom",
      "sourceLine": 11,
      "sourceColumn": 5
    }
  ]
}
```

Statement types covered: `signal_assign`, `constraint_signal_assign`, `var_assign`, `constraint_equality`, `signal_input_declaration`, `signal_output_declaration`, `signal_intermediate_declaration`, `component_declaration`, `var_declaration`, `return`, `assert`, and multi/underscore variants.

## Design

- **Purely additive**: The `--srcmap` flag is opt-in. When not specified, compilation behavior is identical.
- **Zero impact on existing outputs**: Compiled outputs (`.wasm`, `.r1cs`, `.sym`, C++) are bit-identical with and without `--srcmap`. The flag only generates the additional `.srcmap.json` file.
- **Leverages existing infrastructure**: Uses `FileLocation` data already tracked on every AST node — no changes to the parser, type checker, or code generators.

## Changes

- `code_producers/src/source_map.rs` (new, 237 lines) — SourceMap types, AST walker, JSON serialization
- `circom/src/compilation_user.rs` — `generate_source_map()` function
- `circom/src/input_user.rs` — `--srcmap` CLI flag
- `circom/src/main.rs` — wire flag to CompilerConfig
- `program_structure/src/program_library/file_definition.rs` — `get_column()` and `get_file_path()` helpers on FileLibrary

## Testing

- **Cargo workspace tests**: 8/10 pass; 2 failures are pre-existing upstream bugs in `circom_algebra` (confirmed identical on upstream master)
- **Regression test**: Compiles 3 test circuits with/without `--srcmap`, diffs `.r1cs`, `.sym`, and `.wasm` outputs — all bit-identical
- **8 unit tests** for `source_map.rs`: serialization round-trip, JSON structure validation, file deduplication, multi-file maps, signalName omission, file write/read-back
- **No clippy warnings** in modified crates
