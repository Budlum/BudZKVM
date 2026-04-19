# BudZKVM Architecture

BudZKVM is a ZKP-native Layer 1 execution environment designed from first principles for deterministic provability and high performance.

## 1. Programming Language: BudL (.bud)
- **Statically Typed**: Optimized for Goldilocks field arithmetic.
- **ZK-Intrinsics**: Native support for `poseidon()`, `constrain()`, and `emit()`.
- **State Management**: Built-in `storage` blocks and `Map<K, V>` structures.

## 2. Compiler Pipeline
- **Lexer**: Tokenizes source using `logos`.
- **Parser**: Recursive descent parser building an Abstract Syntax Tree (AST).
- **Sema**: Semantic analyzer for symbol resolution and type checking.
- **Codegen**: Translates AST to Bud-ISA bytecode with jump patching.

## 3. Virtual Machine: BudVM
- **Register-based ISA**: 32 general-purpose 64-bit registers.
- **Execution Trace**: Records every state transition (PC, registers, opcodes) for proving.
- **Syscalls**: Host-provided context (sender, block height).
- **Events**: Off-chain log buffer.

## 4. Proving Layer: bud-proof
- **Trace Matrix**: Converts VM trace into a row-major execution matrix.
- **STARK Prover**: Generates deterministic proofs based on field-accumulated trace hashes.
- **Recursive Aggregation**: Combines multiple transaction proofs into a single block proof.

## 5. State Layer: bud-state
- **Merkle Roots**: XOR-based state accumulation for account and storage integrity.
- **Account Model**: Tracking nonces and balances.

## Execution Flow
`Source (.bud)` -> `Bytecode` -> `VM Execution` -> `Trace` -> `STARK Proof` -> `Recursive Aggregation`
