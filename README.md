# BudZKVM: A Production-Grade ZKP-Native Virtual Machine

BudZKVM is a high-performance, verifiable virtual machine and cryptographic proving engine. It features a custom programming language (BudL), a trace-generating virtual machine (BudVM), and a production-grade STARK-based proving system using the **Plonky3** engine.

## 📚 Crafting a ZKVM: The Book
We have written a comprehensive, book-like guide (in Turkish) on how to build a ZKVM from scratch using BudZKVM as the reference implementation. 
👉 [**Check out the docs/ directory**](docs/README.md) to learn about ISA design, Execution Traces, AIR constraints, and Plonky3 integration!

## 🚀 Quick Start

### Prerequisites
- **Nix** with Flakes enabled.

### Enter Development Environment
```bash
nix develop
```

### Deploy a Contract
```bash
cargo run -p bud-cli -- deploy --program example.bud
```

### Call a Contract (with Proof Generation & Verification)
```bash
cargo run -p bud-cli -- call --bytecode example.bud.budc --sender 1 --args 10 --args 20
```

### Direct Run & Verify
```bash
cargo run -p bud-cli -- run --program example.bud --sender 1
```

## 🏗 Technical Architecture (Stage 2)

- **BudL (.bud)**: A domain-specific language for ZK-computations.
- **BudVM**: A 64-bit, 32-register VM generating high-fidelity execution traces.
- **Bud-ISA**: A deterministic instruction set with optimal ZK-friendly encoding.
- **STARK Engine**: 
    - **Plonky3 Integration**: Powered by Polygon's high-performance Plonky3 prover and Goldilocks field.
    - **Wide Trace Architecture**: Separates CPU execution from the Register Access Table for massive performance gains.
    - **29-Column AIR**: Highly optimized Algebraic Intermediate Representation using boolean selectors and sub-clock ordering to enforce strict Read-after-Write (RaW) consistency.
- **Persistence**: File-based state management with account nonce tracking and state-root calculation.

## 🗺 Roadmap

### 🏁 Milestone 1: Core Foundation (COMPLETED)
- [x] BudL Compiler (Lexer, Parser, Sema, Codegen)
- [x] BudVM Execution Engine with 32-register state tracking
- [x] Persistent State Storage (`state.json`)

### 🚀 Milestone 2: ZK Architecture Optimization (COMPLETED)
- [x] Migrate to Plonky3 backend for Goldilocks field performance
- [x] Implement Register Access Table (RAT)
- [x] Enforce Register Consistency via `COL_REG_SAME` and sub-clock ordering
- [x] Transition from 55-column dense trace to 29-column optimized wide trace

### 🛠 Milestone 2: VM & Language Hardening
- [ ] **Gas Metering**: Deterministic cycle counting for DoS protection.
- [ ] **Loops & Recursion**: Bounded loops and tail-recursive call support.
- [ ] **Standard Library**: Native field-friendly Poseidon and Merkle modules.
- [ ] **Advanced Types**: Structs and custom mappings in BudL.

### 🌐 Milestone 3: Expansion & Integration
- [ ] **Recursive Proof Aggregation**: Real recursive STARK verification in AIR.
- [ ] **WASM Verifier**: Compiling the verifier to WASM for browser-side verification.
- [ ] **JSON-RPC Interface**: External API for zkVM interaction.

## 📜 License
Apache-2.0
