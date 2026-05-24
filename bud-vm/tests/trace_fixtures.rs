use bud_isa::{Instruction, Opcode};
use bud_vm::{Step, Vm};

#[derive(Debug)]
struct ExpectedStep {
    pc: usize,
    next_pc: usize,
    opcode: Opcode,
    rd: u8,
    rs1: u8,
    rs2: u8,
    imm: i32,
    src1_val: u64,
    src2_val: u64,
    dst_val: u64,
    registers: &'static [(usize, u64)],
}

fn inst(opcode: Opcode, rd: u8, rs1: u8, rs2: u8, imm: i32) -> u64 {
    Instruction {
        opcode,
        rd,
        rs1,
        rs2,
        imm,
    }
    .encode()
}

fn assert_step(actual: &Step, expected: &ExpectedStep) {
    assert_eq!(actual.pc, expected.pc);
    assert_eq!(actual.next_pc, expected.next_pc);
    assert_eq!(actual.instruction.opcode, expected.opcode);
    assert_eq!(actual.instruction.rd, expected.rd);
    assert_eq!(actual.instruction.rs1, expected.rs1);
    assert_eq!(actual.instruction.rs2, expected.rs2);
    assert_eq!(actual.instruction.imm, expected.imm);
    assert_eq!(actual.src1_idx, expected.rs1);
    assert_eq!(actual.src2_idx, expected.rs2);
    assert_eq!(actual.dst_idx, expected.rd);
    assert_eq!(actual.src1_val, expected.src1_val);
    assert_eq!(actual.src2_val, expected.src2_val);
    assert_eq!(actual.dst_val, expected.dst_val);

    for &(idx, value) in expected.registers {
        assert_eq!(actual.registers[idx], value, "register r{idx}");
    }
}

fn assert_trace(actual: &[Step], expected: &[ExpectedStep]) {
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.iter().zip(expected) {
        assert_step(actual, expected);
    }
}

#[test]
fn arithmetic_trace_fixture_stays_stable() {
    let program = vec![
        inst(Opcode::Load, 1, 0, 0, 7),
        inst(Opcode::Load, 2, 0, 0, 5),
        inst(Opcode::Add, 3, 1, 2, 0),
        inst(Opcode::Sub, 4, 3, 2, 0),
        inst(Opcode::Mul, 5, 4, 1, 0),
        inst(Opcode::Halt, 0, 0, 0, 0),
    ];

    let mut vm = Vm::new(64);
    vm.run(&program).unwrap();

    assert_trace(
        &vm.trace,
        &[
            ExpectedStep {
                pc: 0,
                next_pc: 1,
                opcode: Opcode::Load,
                rd: 1,
                rs1: 0,
                rs2: 0,
                imm: 7,
                src1_val: 0,
                src2_val: 0,
                dst_val: 7,
                registers: &[(1, 7)],
            },
            ExpectedStep {
                pc: 1,
                next_pc: 2,
                opcode: Opcode::Load,
                rd: 2,
                rs1: 0,
                rs2: 0,
                imm: 5,
                src1_val: 0,
                src2_val: 0,
                dst_val: 5,
                registers: &[(1, 7), (2, 5)],
            },
            ExpectedStep {
                pc: 2,
                next_pc: 3,
                opcode: Opcode::Add,
                rd: 3,
                rs1: 1,
                rs2: 2,
                imm: 0,
                src1_val: 7,
                src2_val: 5,
                dst_val: 12,
                registers: &[(1, 7), (2, 5), (3, 12)],
            },
            ExpectedStep {
                pc: 3,
                next_pc: 4,
                opcode: Opcode::Sub,
                rd: 4,
                rs1: 3,
                rs2: 2,
                imm: 0,
                src1_val: 12,
                src2_val: 5,
                dst_val: 7,
                registers: &[(3, 12), (4, 7)],
            },
            ExpectedStep {
                pc: 4,
                next_pc: 5,
                opcode: Opcode::Mul,
                rd: 5,
                rs1: 4,
                rs2: 1,
                imm: 0,
                src1_val: 7,
                src2_val: 7,
                dst_val: 49,
                registers: &[(4, 7), (5, 49)],
            },
            ExpectedStep {
                pc: 5,
                next_pc: 5,
                opcode: Opcode::Halt,
                rd: 0,
                rs1: 0,
                rs2: 0,
                imm: 0,
                src1_val: 0,
                src2_val: 0,
                dst_val: 0,
                registers: &[(5, 49)],
            },
        ],
    );
    assert_eq!(vm.gas_used, 9);
    assert!(vm.halted);
}

#[test]
fn control_flow_trace_fixture_stays_stable() {
    let program = vec![
        inst(Opcode::Load, 1, 0, 0, 1),
        inst(Opcode::Jnz, 0, 1, 0, 2),
        inst(Opcode::Load, 2, 0, 0, 11),
        inst(Opcode::Load, 2, 0, 0, 22),
        inst(Opcode::Jmp, 0, 0, 0, 2),
        inst(Opcode::Load, 3, 0, 0, 99),
    ];

    let mut vm = Vm::new(64);
    let _ = vm.run_receipt(&program);

    assert_trace(
        &vm.trace,
        &[
            ExpectedStep {
                pc: 0,
                next_pc: 1,
                opcode: Opcode::Load,
                rd: 1,
                rs1: 0,
                rs2: 0,
                imm: 1,
                src1_val: 0,
                src2_val: 0,
                dst_val: 1,
                registers: &[(1, 1)],
            },
            ExpectedStep {
                pc: 1,
                next_pc: 3,
                opcode: Opcode::Jnz,
                rd: 0,
                rs1: 1,
                rs2: 0,
                imm: 2,
                src1_val: 1,
                src2_val: 0,
                dst_val: 0,
                registers: &[(1, 1), (2, 0)],
            },
            ExpectedStep {
                pc: 3,
                next_pc: 4,
                opcode: Opcode::Load,
                rd: 2,
                rs1: 0,
                rs2: 0,
                imm: 22,
                src1_val: 0,
                src2_val: 0,
                dst_val: 22,
                registers: &[(1, 1), (2, 22)],
            },
            ExpectedStep {
                pc: 4,
                next_pc: 6,
                opcode: Opcode::Jmp,
                rd: 0,
                rs1: 0,
                rs2: 0,
                imm: 2,
                src1_val: 0,
                src2_val: 0,
                dst_val: 0,
                registers: &[(2, 22), (3, 0)],
            },
        ],
    );
    assert_eq!(vm.pc, 6);
    assert!(vm.halted);
}

#[test]
#[cfg(feature = "experimental")]
fn memory_storage_and_event_trace_fixture_stays_stable() {
    let program = vec![
        inst(Opcode::Load, 1, 0, 0, 8),
        inst(Opcode::Load, 2, 0, 0, 1234),
        inst(Opcode::Store, 0, 1, 2, 0),
        inst(Opcode::Load, 3, 1, 0, 0),
        inst(Opcode::SWrite, 0, 3, 0, 7),
        inst(Opcode::SRead, 4, 0, 0, 7),
        inst(Opcode::Log, 0, 4, 0, 0),
        inst(Opcode::Halt, 0, 0, 0, 0),
    ];

    let mut vm = Vm::new(32);
    vm.run(&program).unwrap();

    assert_trace(
        &vm.trace,
        &[
            ExpectedStep {
                pc: 0,
                next_pc: 1,
                opcode: Opcode::Load,
                rd: 1,
                rs1: 0,
                rs2: 0,
                imm: 8,
                src1_val: 0,
                src2_val: 0,
                dst_val: 8,
                registers: &[(1, 8)],
            },
            ExpectedStep {
                pc: 1,
                next_pc: 2,
                opcode: Opcode::Load,
                rd: 2,
                rs1: 0,
                rs2: 0,
                imm: 1234,
                src1_val: 0,
                src2_val: 0,
                dst_val: 1234,
                registers: &[(2, 1234)],
            },
            ExpectedStep {
                pc: 2,
                next_pc: 3,
                opcode: Opcode::Store,
                rd: 0,
                rs1: 1,
                rs2: 2,
                imm: 0,
                src1_val: 8,
                src2_val: 1234,
                dst_val: 0,
                registers: &[(1, 8), (2, 1234)],
            },
            ExpectedStep {
                pc: 3,
                next_pc: 4,
                opcode: Opcode::Load,
                rd: 3,
                rs1: 1,
                rs2: 0,
                imm: 0,
                src1_val: 8,
                src2_val: 0,
                dst_val: 1234,
                registers: &[(3, 1234)],
            },
            ExpectedStep {
                pc: 4,
                next_pc: 5,
                opcode: Opcode::SWrite,
                rd: 0,
                rs1: 3,
                rs2: 0,
                imm: 7,
                src1_val: 1234,
                src2_val: 0,
                dst_val: 0,
                registers: &[(3, 1234)],
            },
            ExpectedStep {
                pc: 5,
                next_pc: 6,
                opcode: Opcode::SRead,
                rd: 4,
                rs1: 0,
                rs2: 0,
                imm: 7,
                src1_val: 0,
                src2_val: 0,
                dst_val: 1234,
                registers: &[(4, 1234)],
            },
            ExpectedStep {
                pc: 6,
                next_pc: 7,
                opcode: Opcode::Log,
                rd: 0,
                rs1: 4,
                rs2: 0,
                imm: 0,
                src1_val: 1234,
                src2_val: 0,
                dst_val: 0,
                registers: &[(4, 1234)],
            },
            ExpectedStep {
                pc: 7,
                next_pc: 7,
                opcode: Opcode::Halt,
                rd: 0,
                rs1: 0,
                rs2: 0,
                imm: 0,
                src1_val: 0,
                src2_val: 0,
                dst_val: 0,
                registers: &[(4, 1234)],
            },
        ],
    );
    assert_eq!(vm.events, vec![1234]);
    assert_eq!(vm.storage.get(&7), Some(&1234));
}
