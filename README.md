# BudZKVM: ZKP-Native Layer 1 Execution Environment

BudZKVM is a high-performance, verifiable blockchain execution engine. It features a custom programming language (BudL), a trace-generating virtual machine (BudVM), and a STARK-based proving system designed for infinite scalability via recursive proof aggregation.

## 🚀 Quick Start

### Prerequisites
- **Nix** with Flakes enabled.

### Enter Development Environment
```bash
nix develop
```

### Run a Contract
```bash
cargo run -p bud-cli -- run --program example.bud --args 4919 --args 0 --args 0
```

### Run a Batch (Block Simulation)
```bash
cargo run -p bud-cli -- batch --programs example.bud --programs example2.bud
```

## 🏗 Architecture

- **BudL (.bud)**: A domain-specific language for ZK-smart contracts.
- **BudVM**: A 64-bit register-based VM that generates algebraic execution traces.
- **Bud-ISA**: A deterministic instruction set optimized for proving.
- **Prover**: STARK engine converting traces into cryptographic proofs.
- **Recursive Aggregation**: Combining transaction proofs into a single block proof.

## 🗺 Roadmap

### 🏁 Milestone 1: Core Foundation (Current)
- [x] BudL Compiler (Lexer, Parser, Codegen)
- [x] BudVM Execution Engine with Trace Generation
- [x] Basic STARK Proof Skeleton & Recursive Aggregation
- [x] Storage Mappings & Merkle Verification Intrinsics

### 🛠 Milestone 2: Language & VM Enhancement (Q2 2026)
- [ ] **Advanced Types**: Support for `struct`, `enum`, and fixed-point math.
- [ ] **Loops & Recursion**: Implementing bounded loops and tail-call optimization in VM.
- [ ] **Standard Library**: Native modules for EdDSA, SHA3, and JSON parsing.
- [ ] **Gas Metering**: Adding deterministic resource tracking for DoS protection.

### 🌐 Milestone 3: Networking & Consensus (Q3 2026)
- [ ] **P2P Layer**: Implementation of libp2p-based gossip protocol.
- [ ] **Consensus Integration**: Plugging BudVM into CometBFT (Tendermint) for BFT finality.
- [ ] **State Sync**: Snapshot-based fast sync using recursive proof verification.

### 💎 Milestone 4: Ecosystem & Mainnet (Q4 2026)
- [ ] **BudExplorer**: A block explorer visualizing execution traces and STARK proofs.
- [ ] **Web3 Bridge**: EVM-compatible bridge for cross-chain liquidity.
- [ ] **Developer SDK**: VSCode extension for BudL (LSP) and testing framework.
- [ ] **Mainnet Genesis**: Launching the BudZKVM sovereign network.

## 📜 License
Apache-2.0
