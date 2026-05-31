# BudZKVM

BudZKVM is a ZK-native virtual machine, language toolchain, and STARK proving stack built around a small deterministic ISA, a trace-generating VM, and a Plonky3-based prover backend.

The project has successfully transitioned the prover architecture to a production-grade Multi-STARK system using LogUp (fractional sum) interaction arguments, ensuring full register and memory consistency within the Plonky3 0.5.2 backend.

## Production Hardening Achievements (Phases 1-8)

BudZKVM has successfully completed its core **Production Hardening Plan**, transforming the VM and STARK proving system into an audit-ready, sound, and mathematically secure execution environment:

1. **Profile-Based ISA Security (`bud-isa` & `bud-compiler`)**: Introduced structural `IsaProfile` (`Production`, `Experimental`, `Testing`). In the `Production` profile, the compiler and ISA decoder strictly reject experimental or unconstrained opcodes (e.g. storage, poseidon, bitwise, comparison) at compile-time and decode-time, mitigating soundness issues from partially constrained instructions.
2. **Goldilocks-Native Modular Division (`bud-vm` & `plonky3_air.rs`)**: Refactored the `Div` opcode from integer division to Goldilocks field-native modular division (`src1 * inv(src2) mod P`), perfectly aligning the VM execution semantics with the linear constraint layout `rd * rs2 - rs1 == 0` of the AIR.
3. **Robust Cryptographic Proof Envelope (`bud-proof`)**: Wrapped the raw Plonky3 proofs in a versioned, secure `ProofEnvelope` validating key metadata (p3_version, backend_id, fri_params_id) and binding the deterministic Keccak256 hash of the compiled bytecode to the verification pipeline.
4. **Padding Exclusion in Program CTL LogUp**: Integrated a witness column `COL_CPU_ACTIVE` and a preprocessed active indicator, fully aligning the trace degree calculations between prover and verifier and ensuring padding rows do not participate in the LogUp CTL lookup argument.
5. **R0 Register Zero Value Constraint**: Patched a critical soundness gap by strictly constraining R0's write values to `0` in both trace generation and memory/register event LogUp lookup tables.
6. **State Backend & Durability (`bud-state` & `bud-cli`)**: Defined the `StateBackend` trait with strict transaction-like `commit` and `rollback` semantics, and implemented OS-atomic file writes (temporary file sync + rename) to prevent state corruption.
7. **64-Depth Sparse Merkle Tree & Accounts Encapsulation (`bud-state`)**: Replaced flat hash state roots with a secure 64-depth SMT over account IDs. Exposes $O(\log n)$ inclusion and non-membership proofs (`get_account_proof`, `verify_account_proof`). Completely encapsulated the accounts database by making the HashMap private, forcing all state mutations through transactional `StateBackend` APIs.
8. **Pratt Operator Precedence & Panic-Free Parsing (`bud-compiler`)**: Upgraded the recursive descent parser to support mathematical precedence (+, - before *, /), nested parenthesis expression grouping, hexadecimal literals (`0x...`), and comments skipping (`//` and `/* ... */`). Completely replaced panic-based syntax validation with idiomatic `Result<T, CompileError>` propagation.
9. **Transactional Pipeline & Batch Subcommand (`bud-cli`)**: Enforced transaction boundaries (`begin_transaction`, `rollback`, `commit`) on CLI `run_pipeline` executions to preserve nonces and state consistency upon verification failures. Completed the sequential `Batch` CLI command executing and proving a series of programs over a single shared state.

## What Is In This Repository?

BudZKVM is organized as a Rust workspace:

| Crate | Role |
| --- | --- |
| `bud-isa` | Instruction encoding, opcode definitions, and bytecode-level primitives. |
| `bud-vm` | Deterministic VM execution engine that produces execution traces. |
| `bud-compiler` | BudL compiler pipeline: lexer, parser, semantic analysis, and bytecode generation. |
| `bud-proof` | STARK/AIR proving layer. This is where the Plonky3 prover, custom `bud_stark` flow, proof serialization, and AIR constraints live. |
| `bud-cli` | CLI entry point for compiling, deploying, running, proving, and verifying programs. |
| `bud-state` | File-backed state management, account state, nonce tracking, and state-root helpers. |
| `bud-node` | Node-facing integration layer for execution and future network/RPC workflows. |
| `docs` | Turkish book-style documentation for learning the architecture through the BudZKVM codebase. |

## Current Focus

1. Maintain a clean workspace using the Plonky3 0.5.2 generic configuration model.
2. Leverage the `bud_stark` core for production-grade LogUp Cross-Table Lookups (CTL).
3. Ensure Register and Memory consistency via two-phase auxiliary trace accumulators.
4. Optimize constraint degrees (reduced from 7 to 5) for faster proof generation.
5. Expand automated testing for all major opcode families and memory operations.
6. Bound program identity and public inputs for full protocol soundness.

## Book-Style Documentation

The `docs/` directory contains a Turkish, book-style guide that explains BudZKVM as a reference implementation:

- ISA and bytecode design.
- VM execution and trace generation.
- ZK-friendly architecture choices.
- AIR constraints and Plonky3 integration.
- Compiler, CLI, and ecosystem wiring.
- Prover stabilization and testing flow.

Start here: [`docs/README.md`](docs/README.md)

## Quick Start

### Prerequisites

BudZKVM uses Nix for a reproducible development environment.

```bash
nix develop
```

### Build

```bash
cargo check
```

### Run Tests

```bash
cargo test
```

For the prover crate only:

```bash
cargo test -p bud-proof
```

### Development Command Matrix

Common workspace, crate-specific, CLI, and release-check commands are documented in
[`docs/development.md`](docs/development.md). The local CI equivalent is:

```bash
nix develop --command cargo fmt --all -- --check
nix develop --command cargo check
nix develop --command cargo test
nix develop --command python3 scripts/check_docs_links.py
```

### Compile and Run a Program

```bash
cargo run -p bud-cli -- run --program example.bud --sender 1
```

### Deploy a Program

```bash
cargo run -p bud-cli -- deploy --program example.bud
```

### Call a Deployed Program

```bash
cargo run -p bud-cli -- call --bytecode example.bud.budc --sender 1 --args 10 --args 20
```

## Prover Architecture

The proving layer is split into two levels:

1. `plonky3_air.rs` defines the BudZKVM AIR: opcode selectors, PC transition rules, register constraints, halt handling, and field-level transition checks.
2. `bud_stark/` implements the custom STARK proving and verification flow around Plonky3 primitives.

Important prover files:

| File | Responsibility |
| --- | --- |
| `bud-proof/src/plonky3_air.rs` | Main AIR constraints for BudVM execution traces. |
| `bud-proof/src/plonky3_prover.rs` | Adapter from BudZKVM's `ProofSystem` API to the Plonky3/`bud_stark` backend. |
| `bud-proof/src/bud_stark/config.rs` | Central Plonky3 config traits, type aliases, PCS, challenger, and domain wiring. |
| `bud-proof/src/bud_stark/proof.rs` | Proof, commitments, opened values, and serde boundaries. |
| `bud-proof/src/bud_stark/prover.rs` | Main prover flow: commit, challenge, quotient, open. |
| `bud-proof/src/bud_stark/verifier.rs` | Verification flow and proof-shape validation. |
| `bud-proof/src/bud_stark/folder.rs` | Constraint folders for prover and verifier evaluation contexts. |
| `bud-proof/src/bud_stark/sub_builder.rs` | Sub-AIR builder utilities and window-slicing support. |

## Detailed Roadmap

This roadmap is intentionally detailed. BudZKVM is not just a VM, and it is not just a prover wrapper. The long-term goal is a small but complete ZK execution stack: language, bytecode, VM, trace, AIR, prover, proof transport, state integration, and node-facing execution.

### Phase 0: Workspace Baseline and Development Hygiene

Status: mostly complete, continuously maintained.

- [x] Establish a Rust workspace with separate crates for ISA, VM, compiler, proof, CLI, state, and node integration.
- [x] Add a Nix development environment so contributors can enter the same toolchain consistently.
- [x] Keep `cargo check` as the minimum workspace health gate.
- [x] Keep crate boundaries clear enough that prover changes do not require language or CLI rewrites.
- [x] Maintain example Bud programs such as `example.bud`, `example_loop.bud`, and `test_prover.bud`.
- [x] Add a documented command matrix for common development workflows.
- [x] Add CI jobs for `cargo check`, `cargo test`, formatting, and docs link checks.
- [x] Add a contributor guide explaining how to add an opcode from ISA to VM to AIR to tests.
- [x] Add a lightweight release checklist for proof-format changes.

### Phase 1: ISA and Bytecode Foundation

Status: completed.

- [x] Define a compact instruction encoding for BudVM bytecode.
- [x] Implement deterministic opcode definitions in `bud-isa`.
- [x] Support arithmetic instructions needed by the current VM and prover tests.
- [x] Support control-flow instructions needed by compiled BudL programs.
- [x] Support halt semantics at the bytecode level.
- [x] Write a formal opcode reference in `docs/02_isa_ve_bytecode.md`.
- [x] Add golden bytecode tests for every instruction encoding.
- [x] Add invalid-instruction tests that prove decoding failures are deterministic.
- [x] Add versioning metadata to bytecode artifacts.
- [x] Decide which opcodes are part of the stable ISA and which are experimental.
- [x] Add an ISA compatibility policy for future bytecode changes.

### Phase 2: BudVM Execution Engine

Status: completed.

- [x] Implement a 64-bit register-based VM with 32 registers.
- [x] Generate execution traces during VM execution.
- [x] Track program counter, decoded instruction fields, register reads, register writes, and halt state.
- [x] Keep VM behavior deterministic for prover compatibility.
- [x] Provide enough trace data for the current Plonky3 AIR.
- [x] Make gas/cycle accounting explicit and part of VM execution.
- [x] Define exact behavior for invalid memory/register/program-counter access.
- [x] Add tests for halt-after-halt behavior and trace padding.
- [x] Add tests for branch/jump edge cases.
- [x] Add tests for arithmetic overflow semantics.
- [x] Add a VM trace schema document that maps every trace column to its meaning.
- [x] Add trace fixtures so prover tests can compare against stable expected traces.

### Phase 3: BudL Compiler and Language Surface

Status: usable, needs language-hardening work.

- [x] Implement lexer, parser, semantic analysis, and code generation.
- [x] Compile BudL source into BudVM bytecode.
- [x] Support basic program execution through `bud-cli`.
- [x] Support loops and basic control-flow patterns used by current examples.
- [ ] Document the BudL grammar in the docs book.
- [ ] Add compiler snapshot tests for representative programs.
- [x] Add negative tests for syntax and semantic errors.
- [ ] Improve diagnostic messages with source spans.
- [ ] Define integer, field, boolean, and memory semantics precisely.
- [ ] Add structs or records if they remain aligned with the ZK-friendly execution model.
- [ ] Add a small standard library for field-friendly primitives.
- [ ] Decide how much high-level language support should compile to VM opcodes versus builtins.

### Phase 4: Plonky3 0.5.2 Prover Stabilization

Status: in progress, current priority.

- [x] Migrate the prover stack away from the older Plonky3 assumptions.
- [x] Rework `StarkGenericConfig` around Plonky3 0.5.2 concepts: PCS, challenger, domain, commitment, and challenge field.
- [x] Centralize aliases such as `Val<SC>`, `PackedVal<SC>`, `PackedChallenge<SC>`, `Com<SC>`, and `PcsProof<SC>`.
- [x] Remove `no_std` constraints where they block practical prover implementation.
- [x] Enable standard-library features needed by the prover, including boxed closures and parallel-friendly dependencies.
- [x] Restore proof serialization with explicit serde bounds instead of relying on fragile inferred generic bounds.
- [x] Replace placeholder Plonky3 calls in `Plonky3Adapter` with the new `bud_stark` proving and verification API.
- [x] Ensure invalid proof bytes fail verification instead of panicking.
- [x] Keep `cargo check` passing after the migration work.
- [ ] Audit every public type alias in `bud_stark/config.rs` for long-term readability.
- [ ] Add direct tests around proof serialization compatibility.
- [ ] Add tests that exercise verifier failure paths for malformed openings and wrong proof shapes.
- [ ] Decide whether proof structs need manual `Serialize`/`Deserialize` implementations for long-term stability.
- [ ] Add a proof-format version field before treating serialized proofs as stable artifacts.
- [ ] Document the exact Plonky3 version and backend assumptions in the README and docs.

### Phase 5: AIR Constraint Coverage

Status: partially implemented, must be expanded opcode by opcode.

- [x] Implement the basic `BudAir` structure for evaluating VM traces.
- [x] Use selector columns to activate opcode-specific constraints.
- [x] Check core PC transition behavior.
- [x] Check basic arithmetic behavior in prover tests.
- [x] Exercise `ADD`, `SUB`, `MUL`, immediate loading, and halt flows in tests.
- [ ] Audit every opcode in `bud-isa` against `plonky3_air.rs`.
- [ ] Add one prover test per opcode or opcode family.
- [ ] Add negative tests where a tampered trace must fail constraints.
- [ ] Strengthen boolean constraints for selector columns.
- [ ] Prove selector exclusivity: exactly one instruction shape should be active per valid row.
- [ ] Strengthen `COL_IS_HALT` so program termination cannot be forged or weakened by padded rows.
- [ ] Verify bitwise constraints with boolean decomposition rather than native integer intuition.
- [ ] Add range-check strategy for values that are interpreted as small integers.
- [ ] Add public input binding for program identity and final state commitments.

### Phase 6: Two-Phase Trace and Cross-Table Lookup

Status: completed.

- [x] Introduce a two-phase proving flow: main trace first, Fiat-Shamir randomness second, auxiliary trace third.
- [x] Add auxiliary trace plumbing to the prover API.
- [x] Make `PermutationAirBuilder` expose auxiliary windows instead of empty placeholders.
- [x] Update `SubAirBuilder` and sliced builders to forward the relevant builder capabilities.
- [x] Recompose verifier-side auxiliary opening values into challenge field elements.
- [x] Implement real LogUp (fractional sum) accumulator columns for auxiliary trace.
- [x] Define and implement the register event table shape and constraints.
- [x] Define and implement the memory event table shape and constraints.
- [x] Implement CPU-to-register cross-table lookup using 3 challenges ($\alpha, \beta, \gamma$).
- [x] Implement CPU-to-memory cross-table lookup with sorted memory event validation.
- [x] Add LogUp boundary constraints (`when_first_row`, `when_last_row`) for accumulator integrity.
- [x] Fully verify complex stack traces including nested `CALL`, `RET`, `PUSH`, and `POP`.
- [x] Resolve `R0` trace mismatch anomalies and properly synchronize `RET` control-flow constraints with the memory table.
- [ ] Add explicit negative tests where swapped or missing register events fail verification.
- [ ] Optimize fractional sum inversion costs during auxiliary trace generation.

### Phase 7: Proof API, Transport, and Compatibility

Status: functional but not yet final.

- [x] Keep `bud-proof::Proof` as the simple byte-carrying type exposed to the rest of the workspace.
- [x] Serialize internal `bud_stark::Proof<MyConfig>` through bincode in the Plonky3 adapter.
- [x] Deserialize proof bytes during verification.
- [x] Return `false` on invalid proof bytes.
- [ ] Add an explicit proof envelope with version, backend, field, and circuit identifiers.
- [ ] Add tests that old proof versions fail with clear errors once versioning exists.
- [ ] Decide whether bincode remains the long-term transport or only the local development transport.
- [ ] Add optional JSON metadata for CLI inspection without exposing the full proof internals.
- [ ] Add deterministic proof fixture tests when randomness and transcript behavior are stable enough.
- [ ] Add proof size tracking benchmarks.
- [ ] Add verifier-only APIs for node and browser integration.

### Phase 8: CLI and Developer Experience

Status: usable, needs polish and stronger UX.

- [x] Provide CLI flows for running, deploying, and calling Bud programs.
- [x] Connect CLI calls to proof generation and verification.
- [x] Support file-backed state in local workflows.
- [x] Add `bud-cli prove` and `bud-cli verify` as explicit commands if they are not already first-class.
- [ ] Add command output modes: human-readable, JSON, and quiet.
- [ ] Add better error messages for compile, VM, proof, and state failures.
- [ ] Add examples for deploying, calling, and verifying in the README.
- [ ] Add integration tests that run CLI commands against example programs.
- [ ] Add a `--trace` or `--dump-trace` mode for debugging AIR failures.
- [ ] Add a `--proof-out` and `--proof-in` workflow for saving and verifying proofs across commands.
- [ ] Add state reset and state inspect commands for local development.

### Phase 9: State, Accounts, and L1 Integration

Status: partly wired, needs protocol-level clarity.

- [x] Maintain file-backed state for local execution.
- [x] Track account-like state and nonce-related behavior.
- [x] Support BudZKVM as an execution backend concept for contract-call style flows.
- [x] Specify exactly which state fields are committed into public inputs.
- [x] Bind initial state root and final state root into the proof.
- [x] Bind sender, arguments, bytecode hash, gas limit, and execution result into the proof.
- [x] Add replay-protection tests around nonce behavior.
- [x] Add state transition tests where invalid final state must fail verification.
- [ ] Add L1-facing proof verification API boundaries.
- [ ] Add documentation for how BudZKVM execution maps to a transaction lifecycle.
- [ ] Define how proof failures are surfaced to the node layer.

### Phase 10: Performance and Benchmarking

Status: early.

- [ ] Add benchmark harnesses for VM execution, trace generation, proof generation, and verification.
- [ ] Track proof size over representative programs.
- [ ] Track prover time over increasing trace lengths.
- [ ] Track verifier time separately from prover time.
- [ ] Measure auxiliary trace overhead once real lookup accumulators are implemented.
- [ ] Compare dense trace versus split-table trace performance.
- [ ] Add flamegraph-friendly benchmark commands.
- [ ] Evaluate Rayon parallelism and make thread-count behavior configurable.
- [ ] Add performance notes to docs so optimization decisions are preserved.

### Phase 11: Security and Soundness Review

Status: pending deeper prover completion.

- [ ] Review every AIR constraint for missing selector, transition, and boundary conditions.
- [ ] Review halt constraints and trace padding assumptions.
- [ ] Review public input binding so proofs cannot be replayed for different programs or states.
- [ ] Review Fiat-Shamir transcript ordering.
- [ ] Review challenge sampling and auxiliary trace dependency rules.
- [ ] Review proof deserialization for denial-of-service or malformed-input issues.
- [ ] Add property-style tests for VM determinism.
- [ ] Add tampered-trace tests for every major constraint family.
- [ ] Add a security notes document listing known assumptions and non-goals.
- [ ] Prepare an external audit checklist once lookup/permutation constraints are complete.

### Phase 12: Documentation and Learning Material

Status: active.

- [x] Maintain a Turkish book-style documentation track in `docs/`.
- [x] Add a prover stabilization chapter connected to the actual Plonky3 migration work.
- [x] Update the STARK/Plonky3 chapter with the current two-phase prover flow.
- [ ] Add diagrams for the VM-to-trace-to-AIR-to-proof pipeline.
- [ ] Add a full opcode-to-constraint walkthrough.
- [ ] Add a chapter explaining proof serialization and verifier responsibilities.
- [ ] Add a chapter explaining cross-table lookup from first principles using BudZKVM tables.
- [ ] Add debugging guides for common prover errors.
- [ ] Add a "how to add a new opcode" guide that touches every crate.
- [ ] Keep documentation examples synchronized with executable tests.

### Phase 13: Future Expansion

Status: design stage.

- [ ] Add recursive proof aggregation once the base proof system is stable.
- [ ] Add a WASM verifier target for browser or light-client verification.
- [ ] Add JSON-RPC or node-facing APIs for external systems.
- [ ] Add richer BudL standard library primitives such as hash, Merkle, and field utilities.
- [ ] Add proof aggregation for batched contract calls.
- [ ] Add multi-program proof composition if the execution model requires it.
- [ ] Explore alternative PCS/backends only after the Plonky3 0.5.2 path is stable.

## Near-Term Execution Plan

The next concrete development sequence is:

1. Finish Phase 4 cleanup around Plonky3 config aliases, serde boundaries, and proof compatibility.
2. Continue Phase 5 by auditing `plonky3_air.rs` opcode by opcode and implementing bitwise operation lookup tables.
3. Add negative prover tests that intentionally violate register consistency, halt behavior, and proof shape.
4. Bind program identity and state commitments into public inputs.
5. Expand documentation in parallel so each prover milestone has a matching explanation in `docs/`.

## Verification Status

The current stabilization baseline is expected to pass:

```bash
cargo check
cargo test -p bud-proof
```

## 👥 Team & Advisors

Budlum Core is an open research-oriented Layer-1 blockchain project.  
For protocol discussions, ecosystem opportunities, advisory inquiries, product collaboration, or investment-related communication, you can reach the relevant team members below.

| Role | Contact | Social Links |
|---|---|---|
| Author & Lead Protocol Engineer | Radeonares32 | [GitHub](https://github.com/Radeonares32) |
| Advisor | Avalanche Alpha | [X / Twitter](https://x.com/AvalancheAlpha) · [Telegram](https://t.me/BCGamingExpert) |
| Product Manager | Demhat Kara | [LinkedIn](https://www.linkedin.com/in/demhat-kara-11b20925a/) · [X / Twitter](https://x.com/DemhatKara3) · [Telegram](https://t.me/DemhatKara) |

For general technical contributions, please open an issue, discussion, or pull request directly on GitHub.

The prover test suite currently covers successful proof generation/verification for simple arithmetic traces, immediate loading, proof byte round-trips, and invalid proof byte rejection.

## License

Apache-2.0
