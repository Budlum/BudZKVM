use bud_isa::{Instruction, Opcode};
use bud_proof::plonky3_air::{BudAir, TRACE_WIDTH};
use bud_vm::Vm;
use p3_goldilocks::Goldilocks;
use p3_matrix::dense::RowMajorMatrix;

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

#[test]
fn test_tampered_pc_violates_constraints() {
    let program = vec![
        inst(Opcode::Add, 1, 2, 3, 0),
        inst(Opcode::Halt, 0, 0, 0, 0),
    ];
    let mut vm = Vm::new(64);
    vm.registers[2] = 10;
    vm.registers[3] = 20;
    let _receipt = vm.run_receipt(&program);

    // Let's create a main trace matrix but tamper with the PC column!
    let mut values = vec![Goldilocks::new(0); 16 * TRACE_WIDTH];
    for (i, step) in vm.trace.iter().enumerate() {
        let row_start = i * TRACE_WIDTH;
        values[row_start] = Goldilocks::new(i as u64); // clk
        values[row_start + 1] = Goldilocks::new(999); // TAMPERED PC! (instead of step.pc)
        values[row_start + 2] = Goldilocks::new(step.instruction.opcode as u64);
        values[row_start + 3] = Goldilocks::new(step.dst_idx as u64);
        values[row_start + 11 + step.instruction.opcode as usize] = Goldilocks::new(1);
        // selector
    }

    let matrix = RowMajorMatrix::new(values, TRACE_WIDTH);
    let air = BudAir {
        num_steps: vm.trace.len(),
        program,
    };

    // Evaluating constraints on tampered trace should fail/panic!
    let res = std::panic::catch_unwind(|| {
        let public_inputs = vec![Goldilocks::new(0); 48];
        p3_air::check_constraints(&air, &matrix, &public_inputs);
    });
    assert!(res.is_err());
}
