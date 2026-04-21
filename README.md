# BudZKVM: A Production-Grade ZKP-Native Virtual Machine

BudZKVM is a high-performance, verifiable virtual machine and cryptographic proving engine. It features a custom programming language (BudL), a trace-generating virtual machine (BudVM), and a production-grade STARK-based proving system using the Winterfell engine.

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

## 🏗 Technical Architecture

- **BudL (.bud)**: A domain-specific language for ZK-computations.
- **BudVM**: A 64-bit, 32-register VM generating high-fidelity execution traces.
- **Bud-ISA**: A deterministic instruction set with 31+ opcodes.
- **STARK Engine**: 
    - **Winterfell Integration**: Powered by the industry-standard Winterfell 0.7 prover.
    - **55-Column AIR**: Comprehensive Algebraic Intermediate Representation covering arithmetic (ADD, SUB, MUL), comparisons (EQ, LT, GT, etc.), storage, and hashing.
    - **Combined Constraints**: Optimized degree-2 transition constraints using opcode selectors.
- **Persistence**: File-based state management with account nonce tracking and state-root calculation.

## 🗺 Roadmap

### 🏁 Milestone 1: Core Foundation (COMPLETED)
- [x] BudL Compiler (Lexer, Parser, Sema, Codegen)
- [x] BudVM Execution Engine with 32-register state tracking
- [x] Full STARK AIR (55-column trace) with all arithmetic/comparison constraints
- [x] Automated Proof Generation & Verification Pipeline
- [x] Persistent State Storage (`state.json`)

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
