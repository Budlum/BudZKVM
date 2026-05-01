# Development Workflow

This page is the day-to-day command matrix for BudZKVM contributors. Run commands from the
workspace root unless a command says otherwise.

## Environment

| Task | Command | Notes |
| --- | --- | --- |
| Enter the reproducible shell | `nix develop` | Provides the Rust toolchain used by the workspace. |
| Run a one-off command in the shell | `nix develop --command <command>` | Useful for CI parity without entering an interactive shell. |
| Inspect workspace crates | `cargo metadata --no-deps` | Confirms crate membership and package names. |

## Health Gates

| Task | Command | Notes |
| --- | --- | --- |
| Format check | `cargo fmt --all -- --check` | Required before opening a PR. |
| Format files | `cargo fmt --all` | May touch many Rust files; review the diff. |
| Minimum compile gate | `cargo check` | README Phase 0 health gate. |
| Full unit test suite | `cargo test` | Runs all workspace tests. |
| Prover-only tests | `cargo test -p bud-proof` | Fast feedback for AIR/prover work. |
| Docs link check | `python3 scripts/check_docs_links.py` | Validates local Markdown links. |

These gates are mirrored in `.github/workflows/ci.yml` so local development and CI exercise the
same baseline checks.

## CLI Workflows

| Task | Command | Notes |
| --- | --- | --- |
| Compile and run a Bud program | `cargo run -p bud-cli -- run --program example.bud --sender 1` | Uses the local VM path. |
| Deploy a Bud program | `cargo run -p bud-cli -- deploy --program example.bud` | Emits a `.budc` bytecode artifact. |
| Call deployed bytecode | `cargo run -p bud-cli -- call --bytecode example.bud.budc --sender 1 --args 10 --args 20` | Exercises state and call handling. |

## Focused Crate Commands

| Area | Command | When to use it |
| --- | --- | --- |
| ISA | `cargo test -p bud-isa` | Instruction encoding/decoding changes. |
| VM | `cargo test -p bud-vm` | Execution semantics, trace generation, gas, stack, memory. |
| Compiler | `cargo test -p bud-compiler` | Lexer, parser, semantic analysis, codegen. |
| Prover | `cargo test -p bud-proof` | Plonky3 adapter, AIR, proof serialization, verifier behavior. |
| CLI | `cargo test -p bud-cli` | CLI command behavior and integration helpers. |
| State | `cargo test -p bud-state` | Account/state-root behavior. |

## Before Sending Changes

Run this local equivalent of CI:

```bash
nix develop --command cargo fmt --all -- --check
nix develop --command cargo check
nix develop --command cargo test
nix develop --command python3 scripts/check_docs_links.py
```

For prover work, also run:

```bash
nix develop --command cargo test -p bud-proof
```
