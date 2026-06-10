# BudZKVM

BudZKVM is a ZK-native virtual machine, language toolchain, and STARK proving stack built around a small deterministic ISA (31 opcodes), a trace-generating VM, and a Plonky3 0.5.2-based prover backend.

**Faz 0 completed** — 31/31 opcodes production-ready, full AIR constraint coverage, 51 tests (8 negative), 0 failures.

## Production Hardening Achievements

BudZKVM has completed its core production hardening plan:

1. **All 31 Opcodes Production-Ready**: Comparison (64-bit decomposition + equality prefix flags), Bitwise (bit decomposition + algebraic equivalence), Poseidon4 hash (alpha=7, Goldilocks), Storage (STORAGE_BASE address-space partitioning via memory LogUp), VerifyMerkle (poseidon4_hash-based 64-depth).
2. **Profile-Based ISA Security**: `IsaProfile` gates experimental opcodes at compile/decode time. Currently all opcodes are production (0 experimental).
3. **Soundness via Inverse Witnesses**: Div, Inv, Eq/Neq, Jnz, and Not all use arithmetic inverse witness patterns for zero/non-zero detection.
4. **R0 Hardware Zero**: Hardwired R0 constraint enforced at trace generation and AIR level.
5. **Padding Isolation**: `COL_CPU_ACTIVE` excludes padding rows from LogUp CTL lookups.
6. **3-Table LogUp CTL**: Register, Memory+Storage, and Program tables cross-verified via LogUp fractional sums.
7. **DoS-Protected Serialization**: `postcard` with bounded deserialization (10 MB limit).
8. **Structured Tracing**: `RUST_LOG=info` pipeline-wide tracing (compiler, VM, prover, CLI).
9. **8 Negative Soundness Tests**: Tampered comparison, bitwise, poseidon S-box, storage, PC, public inputs, program, and invalid proof bytes.
10. **64-Depth Sparse Merkle Tree**: File-backed state with SMT inclusion/non-membership proofs, domain-separated state root hashing.
11. **CI Pipeline**: fmt + check + clippy + test + docs links + cargo deny.

## What Is In This Repository?

| Crate | Role |
| --- | --- |
| `bud-isa` | Instruction encoding, opcode definitions (31 opcodes), bytecode primitives |
| `bud-vm` | Deterministic VM (32 reg, 64-bit, gas-metered), execution trace generation |
| `bud-compiler` | BudL compiler: lexer, recursive-descent parser, semantic analysis, codegen |
| `bud-proof` | Plonky3 0.5.2 STARK prover, 354-column trace matrix, LogUp CTL, custom `bud_stark` |
| `bud-cli` | CLI: compile, deploy, run, prove, verify, batch (with structured tracing) |
| `bud-state` | File-backed state, 64-depth SMT, account model, transactional commit/rollback |
| `bud-node` | Node integration layer (placeholder for future P2P/RPC) |
| `docs` | Turkish book-style documentation (10 chapters + 3 guides) |

## Quick Start

```bash
nix develop                           # Enter reproducible dev environment
cargo check                           # Build check
cargo test                            # 51 tests
RUST_LOG=info cargo run -p bud-cli -- run --program example.bud --sender 1
```

## Book-Style Documentation

The `docs/` directory contains a Turkish, book-style guide that teaches ZKVM architecture through BudZKVM:

1. [Giriş — ZKVM Nedir?](docs/01_giris.md)
2. [Komut Seti Mimarisi (ISA)](docs/02_isa_ve_bytecode.md)
3. [Sanal Makine İnşası (VM)](docs/03_virtual_machine.md)
4. [ZK Dostu Mimari](docs/04_zk_friendly_architecture.md)
5. [STARK, AIR ve Plonky3](docs/05_stark_ve_plonky3.md)
6. [Derleyici ve Ekosistem](docs/06_compiler_ve_ekosistem.md)
7. [Prover Stabilizasyonu](docs/07_prover_stabilizasyonu_ve_testler.md)
8. [Production Hardening & Soundness](docs/08_production_hardening_ve_soundness.md)
9. [Faz 0 — Stabilizasyon ve Üretime Geçiş](docs/09_faz0_stabilizasyon.md)

Start here: [`docs/README.md`](docs/README.md)

## Prover Architecture

```
VM Trace (Vec<Step>)
  └─→ trace_matrix() → 354-column RowMajorMatrix<Goldilocks>
       ├─ Columns 0-64:    Core + Selectors + Register + Memory + Soundness
       ├─ Columns 65-257:  Comparison/Bitwise (64-bit decomposition + eq flags)
       └─ Columns 258-353: Poseidon (4-round state + S-box intermediates)
  └─→ aux_trace_generator() → 3-column LogUp accumulators (Reg, Mem+Stor, Prog)
  └─→ prove_with_preprocessed() → Proof
  └─→ postcard::serialize → ProofEnvelope
```

| File | Responsibility |
| --- | --- |
| `bud-proof/src/plonky3_air.rs` | Main AIR constraints (eval function, 354 columns) |
| `bud-proof/src/plonky3_prover.rs` | Adapter: trace matrix, aux trace, prove/verify |
| `bud-proof/src/bud_stark/prover.rs` | Prover flow: commit, challenge, quotient, open |
| `bud-proof/src/bud_stark/verifier.rs` | Verification flow |
| `bud-proof/src/bud_stark/folder.rs` | Constraint folders (prover/verifier) |
| `bud-proof/src/bud_stark/config.rs` | StarkGenericConfig type aliases |

## Detailed Roadmap

### ✅ Phase 0: Workspace Baseline (complete)
- [x] Rust workspace, Nix env, CI (fmt+check+clippy+test+docs+deny)
- [x] Example Bud programs, command matrix
- [x] Opcode contribution guide, proof format checklist

### ✅ Phase 1: ISA & Bytecode (complete)
- [x] 31 opcodes, deterministic encoding, production/experimental profiles
- [x] All opcodes now production (0 experimental)

### ✅ Phase 2: VM Execution Engine (complete)
- [x] 64-bit register-based VM, 32 regs, gas metering
- [x] Execution trace with full Step struct
- [x] Poseidon4 hash, VerifyMerkle (64-depth), Storage (HashMap)

### ✅ Phase 3: BudL Compiler (complete)
- [x] Lexer (logos), recursive-descent parser, sema, codegen
- [x] while/for loops, operator precedence, comments
- [x] User-defined function calls (Call/Ret, caller-saved registers)
- [x] Static Type System (Semantic Analyzer, u64/bool/field, return types)
- [x] Struct support and Dynamic Heap Memory (r31 HEAP_PTR)
- [ ] Pattern matching (`match` expressions) and better error spans

### ✅ Phase 4: Plonky3 0.5.2 Prover (complete)
- [x] StarkGenericConfig, PCS, challenger wiring
- [x] postcard serialization (bounded, DoS-safe)
- [x] ProofEnvelope with versioning

### ✅ Phase 5: AIR Constraint Coverage (complete)
- [x] All 31 opcodes constrained: arithmetic, comparison, bitwise, poseidon, storage, merkle
- [x] Selector exclusivity, booleanity, PC transitions
- [x] 8 negative soundness tests

### ✅ Phase 6: LogUp Cross-Table Lookup (complete)
- [x] 3-table LogUp: Register, Memory+Storage, Program CTL
- [x] Storage via STORAGE_BASE address partitioning
- [x] LogUp boundary constraints (first_row=0, last_row=0)

### ✅ Phase 7: Proof API & Transport (complete)
- [x] ProofEnvelope with version, backend, FRI params
- [x] Public input Keccak256 hash binding
- [x] Bounded deserialization

### ✅ Phase 8: CLI & Developer Experience (functional)
- [x] Run, prove, batch, deploy, call, verify commands
- [x] Structured tracing (RUST_LOG)
- [ ] JSON output mode, trace dump, state inspect

### Phase 9: State, Accounts, L1 Integration (in progress)
- [x] File-backed state, 64-depth SMT, account model
- [x] Transactional commit/rollback, atomic state save
- [ ] L1-facing JSON-RPC node APIs

### Phase 10: Performance & Benchmarking (planned)
- [ ] Criterion benchmarks, proof size/time tracking
- [ ] Rayon parallelism tuning

### Phase 11: Security Audit (planned)
- [ ] External audit, full constraint review, fuzzing

### Phase 12: Docs & Learning Material (active)
- [x] 10-chapter Turkish book + 3 developer guides
- [ ] Diagrams, opcode-to-constraint walkthrough, debugging guides

## Near-Term Plan (Faz 2 - Compiler Olgunlaştırma)

1. **Error Spans**: Adding line/column numbers to compile errors.
2. **Match Expressions**: Adding pattern matching.
3. **Witness Variables**: Private ZK inputs support.
4. Benchmark suite (criterion) — proving/verification time, proof size.
5. Prover parallelism optimization (Rayon).

## Verification Status

```bash
cargo check --workspace --all-targets     # Clean
cargo clippy --workspace -- -D warnings   # Clean
cargo test --workspace                    # 51 passed, 0 failed
cargo fmt --all -- --check                # Clean
python3 scripts/check_docs_links.py       # 16 files, all links valid
```

## License

MIT
