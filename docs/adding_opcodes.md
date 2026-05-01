# Adding an Opcode

This guide is the contributor checklist for adding a BudVM opcode without breaking the ISA, VM,
trace, AIR, and proof stack contract.

## 1. Define the ISA Surface

Update `bud-isa/src/lib.rs`:

- Add the opcode variant to `Opcode`.
- Assign a stable discriminant.
- Update `Instruction::decode` so the raw byte maps to the new variant.
- Add or update encoding/decoding tests if the opcode changes bytecode behavior.

Keep the discriminant stable once bytecode artifacts may depend on it. If a value is experimental,
document that status in `docs/02_isa_ve_bytecode.md`.

## 2. Implement VM Semantics

Update `bud-vm/src/lib.rs`:

- Add the opcode arm in `Vm::step`.
- Define register reads, register writes, `dst_val`, and `next_pc`.
- Decide how the opcode interacts with memory, storage, stack, gas, and halt behavior.
- Add VM tests for normal behavior and edge cases.

The VM trace must contain enough information for the AIR to verify the step. If the AIR needs a
new witness value, add it to `Step` and the trace matrix deliberately.

## 3. Emit It From the Compiler or CLI

If the opcode is user-facing, update the compiler pipeline:

- Parser/AST changes in `bud-compiler/src/ast.rs` and `parser.rs`.
- Semantic validation in `bud-compiler/src/sema.rs`.
- Bytecode generation in `bud-compiler/src/codegen.rs`.
- CLI examples or fixtures when helpful.

Opcodes can exist in the VM before the language exposes them, but the docs should say whether the
opcode is internal, experimental, or stable.

## 4. Add Trace Columns or Selectors

Update `bud-proof/src/plonky3_air.rs` and `bud-proof/src/plonky3_prover.rs`:

- Add selector columns only when existing selectors cannot represent the opcode.
- Populate the selector in `trace_matrix`.
- Populate any new witness columns.
- Keep trace padding and halt rows consistent.
- Update register, memory, or lookup events if the opcode introduces new reads/writes.

Every new column should have a clear meaning in the trace schema docs before it becomes stable.

## 5. Add AIR Constraints

In `BudAir::eval`:

- Gate opcode-specific equations with the opcode selector.
- Constrain `next_pc` behavior.
- Constrain destination values and side effects.
- Add boolean/range constraints when a value is meant to be small or binary.
- Update permutation/lookup constraints if the opcode reads or writes shared tables.

The constraint must reject a tampered trace, not just accept the honest trace.

## 6. Add Tests

At minimum:

- `bud-isa` encoding/decoding coverage for the opcode.
- `bud-vm` execution coverage.
- `bud-proof` positive prover test.
- A negative prover/verifier test when the AIR is meant to reject a tampered witness.
- Compiler snapshot or integration test if BudL emits the opcode.

## 7. Update Documentation

Update:

- `README.md` roadmap status if the opcode closes a roadmap item.
- `docs/02_isa_ve_bytecode.md` for opcode format and stability.
- `docs/03_virtual_machine.md` for VM semantics when needed.
- `docs/05_stark_ve_plonky3.md` or `docs/07_prover_stabilizasyonu_ve_testler.md` for AIR/prover behavior.

Run the local CI equivalent from `docs/development.md` before sending the change.
