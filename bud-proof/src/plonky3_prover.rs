use crate::adapter::{Proof, ProverAdapter};
use crate::bud_stark::{prove, verify as stark_verify, StarkConfig};
use crate::plonky3_air::*;
use bud_vm::Step;
use p3_challenger::{HashChallenger, SerializingChallenger64};
use p3_commit::ExtensionMmcs;
use p3_dft::Radix2DitParallel;
use p3_field::extension::BinomialExtensionField;
use p3_field::{Field, PrimeCharacteristicRing};
use p3_fri::{create_test_fri_params, TwoAdicFriPcs};
use p3_goldilocks::Goldilocks;
use p3_keccak::Keccak256Hash;
use p3_matrix::dense::RowMajorMatrix;
use p3_merkle_tree::MerkleTreeMmcs;
use p3_symmetric::{CompressionFunctionFromHasher, SerializingHasher};
use std::boxed::Box;

type MyExtensionField = BinomialExtensionField<Goldilocks, 2>;
type MyHasher = SerializingHasher<Keccak256Hash>;
type MyCompress = CompressionFunctionFromHasher<Keccak256Hash, 2, 32>;
type MyMmcs = MerkleTreeMmcs<Goldilocks, u8, MyHasher, MyCompress, 2, 32>;
type MyChallengeMmcs = ExtensionMmcs<Goldilocks, MyExtensionField, MyMmcs>;
type MyPcs = TwoAdicFriPcs<Goldilocks, Radix2DitParallel<Goldilocks>, MyMmcs, MyChallengeMmcs>;
type MyChallenger = SerializingChallenger64<Goldilocks, HashChallenger<u8, Keccak256Hash, 32>>;
type MyConfig = StarkConfig<MyPcs, MyExtensionField, MyChallenger>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RegEvent {
    clk: u64,
    idx: u64,
    val: u64,
    is_write: bool,
    sub_clk: u8,
}

#[derive(Clone, Copy)]
struct MemEvent {
    clk: u64,
    addr: u64,
    val: u64,
    is_write: bool,
}

pub struct Plonky3Adapter;

fn build_config() -> MyConfig {
    let hash = MyHasher::new(Keccak256Hash {});
    let compress = MyCompress::new(Keccak256Hash {});
    let val_mmcs = MyMmcs::new(hash, compress, 0);
    let challenge_mmcs = MyChallengeMmcs::new(val_mmcs.clone());
    let fri_params = create_test_fri_params(challenge_mmcs, 0);
    let inner_challenger = HashChallenger::<u8, Keccak256Hash, 32>::new(vec![], Keccak256Hash {});
    let challenger = MyChallenger::new(inner_challenger);
    let dft = Radix2DitParallel::default();
    let pcs = MyPcs::new(dft, val_mmcs, fri_params);
    MyConfig::new(pcs, challenger)
}

fn initial_registers(trace: &[Step]) -> [u64; 32] {
    let mut registers = [0u64; 32];
    if let Some(first_step) = trace.first() {
        registers = first_step.registers;
        registers[first_step.src1_idx as usize] = first_step.src1_val;
        registers[first_step.src2_idx as usize] = first_step.src2_val;
    }
    registers
}

fn register_events(trace: &[Step]) -> Vec<RegEvent> {
    let mut events = Vec::new();
    let initial_registers = initial_registers(trace);

    for i in 0..32 {
        events.push(RegEvent {
            clk: 0,
            idx: i,
            val: initial_registers[i as usize],
            is_write: true,
            sub_clk: 0,
        });
    }

    for (i, step) in trace.iter().enumerate() {
        let clk = i as u64;
        events.push(RegEvent {
            clk,
            idx: step.src1_idx as u64,
            val: step.src1_val,
            is_write: false,
            sub_clk: 1,
        });
        events.push(RegEvent {
            clk,
            idx: step.src2_idx as u64,
            val: step.src2_val,
            is_write: false,
            sub_clk: 2,
        });
        events.push(RegEvent {
            clk,
            idx: step.dst_idx as u64,
            val: step.dst_val,
            is_write: true,
            sub_clk: 3,
        });
    }

    events.sort_by_key(|e| (e.idx, e.clk, e.sub_clk));
    events
}

fn memory_events(trace: &[Step]) -> Vec<MemEvent> {
    let mut events = Vec::new();
    for (i, step) in trace.iter().enumerate() {
        if let Some(addr) = step.memory_addr {
            events.push(MemEvent {
                clk: i as u64,
                addr: addr as u64,
                val: step.memory_val.unwrap_or(0),
                is_write: step.is_memory_write,
            });
        }
    }
    events.sort_by_key(|e| (e.addr, e.clk));
    events
}

fn trace_matrix(trace: &[Step]) -> (RowMajorMatrix<Goldilocks>, usize) {
    let events = register_events(trace);
    let mem_events = memory_events(trace);
    let n_cpu = trace.len();
    let n_reg = events.len();
    let n_mem = mem_events.len();
    let mut num_rows = n_cpu.max(n_reg).max(n_mem).next_power_of_two();
    if num_rows < 16 {
        num_rows = 16;
    }

    let mut values = vec![Goldilocks::new(0); num_rows * TRACE_WIDTH];

    for (i, step) in trace.iter().enumerate() {
        let row_start = i * TRACE_WIDTH;
        let op = step.instruction.opcode as u8;
        values[row_start + COL_CLK] = Goldilocks::new(i as u64);
        values[row_start + COL_PC] = Goldilocks::new(step.pc as u64);
        values[row_start + COL_OPCODE] = Goldilocks::new(op as u64);
        values[row_start + COL_RD_IDX] = Goldilocks::new(step.dst_idx as u64);
        values[row_start + COL_RS1_IDX] = Goldilocks::new(step.src1_idx as u64);
        values[row_start + COL_RS2_IDX] = Goldilocks::new(step.src2_idx as u64);
        values[row_start + COL_RS1_VAL] = Goldilocks::new(step.src1_val);
        values[row_start + COL_RS2_VAL] = Goldilocks::new(step.src2_val);
        values[row_start + COL_RD_VAL_NEW] = Goldilocks::new(step.dst_val);
        values[row_start + COL_NEXT_PC] = Goldilocks::new(step.next_pc as u64);

        let imm = step.instruction.imm;
        values[row_start + COL_IMM] = if imm < 0 {
            Goldilocks::new(0) - Goldilocks::new((-imm) as u64)
        } else {
            Goldilocks::new(imm as u64)
        };

        match op {
            0x01 => values[row_start + COL_IS_ADD] = Goldilocks::new(1),
            0x02 => values[row_start + COL_IS_SUB] = Goldilocks::new(1),
            0x03 => values[row_start + COL_IS_MUL] = Goldilocks::new(1),
            0x04 => values[row_start + COL_IS_DIV] = Goldilocks::new(1),
            0x05 => values[row_start + COL_IS_INV] = Goldilocks::new(1),
            0x06 => values[row_start + COL_IS_AND] = Goldilocks::new(1),
            0x07 => values[row_start + COL_IS_OR] = Goldilocks::new(1),
            0x08 => values[row_start + COL_IS_XOR] = Goldilocks::new(1),
            0x09 => values[row_start + COL_IS_NOT] = Goldilocks::new(1),
            0x0A => values[row_start + COL_IS_EQ] = Goldilocks::new(1),
            0x0B => values[row_start + COL_IS_NEQ] = Goldilocks::new(1),
            0x0C => values[row_start + COL_IS_LT] = Goldilocks::new(1),
            0x0D => values[row_start + COL_IS_GT] = Goldilocks::new(1),
            0x0E => values[row_start + COL_IS_LTE] = Goldilocks::new(1),
            0x0F => values[row_start + COL_IS_GTE] = Goldilocks::new(1),
            0x10 => values[row_start + COL_IS_JMP] = Goldilocks::new(1),
            0x11 => {
                values[row_start + COL_IS_JNZ] = Goldilocks::new(1);
                values[row_start + COL_JNZ_COND] = if step.src1_val != 0 {
                    Goldilocks::new(1)
                } else {
                    Goldilocks::new(0)
                };
            }
            0x12 => values[row_start + COL_IS_CALL] = Goldilocks::new(1),
            0x13 => values[row_start + COL_IS_RET] = Goldilocks::new(1),
            0x14 => values[row_start + COL_IS_LOAD] = Goldilocks::new(1),
            0x15 => values[row_start + COL_IS_STORE] = Goldilocks::new(1),
            0x16 => values[row_start + COL_IS_PUSH] = Goldilocks::new(1),
            0x17 => values[row_start + COL_IS_POP] = Goldilocks::new(1),
            0x18 => values[row_start + COL_IS_ASSERT] = Goldilocks::new(1),
            0x19 => values[row_start + COL_IS_POSEIDON] = Goldilocks::new(1),
            0x1A => values[row_start + COL_IS_LOG] = Goldilocks::new(1),
            0x1B => values[row_start + COL_IS_SREAD] = Goldilocks::new(1),
            0x1C => values[row_start + COL_IS_SWRITE] = Goldilocks::new(1),
            0x1D => values[row_start + COL_IS_SYSCALL] = Goldilocks::new(1),
            0x1E => values[row_start + COL_IS_VERIFY_MERKLE] = Goldilocks::new(1),
            0x00 => values[row_start + COL_IS_HALT] = Goldilocks::new(1),
            _ => {}
        }
    }

    for i in n_cpu..num_rows {
        let row_start = i * TRACE_WIDTH;
        values[row_start + COL_CLK] = Goldilocks::new(i as u64);
        values[row_start + COL_IS_HALT] = Goldilocks::new(1);
        if n_cpu > 0 {
            let last_pc = trace[n_cpu - 1].next_pc as u64;
            values[row_start + COL_PC] = Goldilocks::new(last_pc);
            values[row_start + COL_NEXT_PC] = Goldilocks::new(last_pc);
        }
    }

    for (i, e) in events.iter().enumerate() {
        let row_start = i * TRACE_WIDTH;
        values[row_start + COL_REG_CLK] = Goldilocks::new(e.clk);
        values[row_start + COL_REG_IDX] = Goldilocks::new(e.idx);
        values[row_start + COL_REG_VAL] = Goldilocks::new(e.val);
        values[row_start + COL_REG_IS_WRITE] = if e.is_write {
            Goldilocks::new(1)
        } else {
            Goldilocks::new(0)
        };
        values[row_start + COL_REG_ACTIVE] = Goldilocks::new(1);

        if i < n_reg - 1 && events[i + 1].idx == e.idx {
            values[row_start + COL_REG_SAME] = Goldilocks::new(1);
        }
    }

    for (i, e) in mem_events.iter().enumerate() {
        let row_start = i * TRACE_WIDTH;
        values[row_start + COL_MEM_CLK] = Goldilocks::new(e.clk);
        values[row_start + COL_MEM_ADDR] = Goldilocks::new(e.addr);
        values[row_start + COL_MEM_VAL] = Goldilocks::new(e.val);
        values[row_start + COL_MEM_IS_WRITE] = if e.is_write {
            Goldilocks::new(1)
        } else {
            Goldilocks::new(0)
        };
        values[row_start + COL_MEM_ACTIVE] = Goldilocks::new(1);

        if i < n_mem - 1 && mem_events[i + 1].addr == e.addr {
            values[row_start + COL_MEM_SAME] = Goldilocks::new(1);
        }
    }

    (RowMajorMatrix::new(values, TRACE_WIDTH), num_rows)
}

fn register_term(
    alpha: MyExtensionField,
    beta: MyExtensionField,
    table_id: Goldilocks,
    clk: Goldilocks,
    idx: Goldilocks,
    val: Goldilocks,
    is_write: Goldilocks,
) -> MyExtensionField {
    let b2 = beta * beta;
    let b3 = b2 * beta;
    let b4 = b3 * beta;
    let b5 = b4 * beta;

    alpha
        + beta * MyExtensionField::from(table_id)
        + b2 * MyExtensionField::from(clk)
        + b3 * MyExtensionField::from(idx)
        + b4 * MyExtensionField::from(val)
        + b5 * MyExtensionField::from(is_write)
}

fn aux_trace_generator(
    main_trace: RowMajorMatrix<Goldilocks>,
    trace_len: usize,
) -> Box<dyn FnOnce(&[MyExtensionField]) -> RowMajorMatrix<Goldilocks>> {
    Box::new(
        move |rand: &[MyExtensionField]| -> RowMajorMatrix<Goldilocks> {
            let alpha = rand[0];
            let beta = rand[1];
            let gamma = rand[2];
            let mut aux_values = vec![MyExtensionField::ZERO; trace_len * 2];

            for i in 0..trace_len.saturating_sub(1) {
                let row_start = i * TRACE_WIDTH;
                let is_cpu = main_trace.values[row_start + COL_IS_ADD]
                    + main_trace.values[row_start + COL_IS_SUB]
                    + main_trace.values[row_start + COL_IS_MUL]
                    + main_trace.values[row_start + COL_IS_DIV]
                    + main_trace.values[row_start + COL_IS_INV]
                    + main_trace.values[row_start + COL_IS_AND]
                    + main_trace.values[row_start + COL_IS_OR]
                    + main_trace.values[row_start + COL_IS_XOR]
                    + main_trace.values[row_start + COL_IS_NOT]
                    + main_trace.values[row_start + COL_IS_EQ]
                    + main_trace.values[row_start + COL_IS_NEQ]
                    + main_trace.values[row_start + COL_IS_LT]
                    + main_trace.values[row_start + COL_IS_GT]
                    + main_trace.values[row_start + COL_IS_LTE]
                    + main_trace.values[row_start + COL_IS_GTE]
                    + main_trace.values[row_start + COL_IS_JMP]
                    + main_trace.values[row_start + COL_IS_JNZ]
                    + main_trace.values[row_start + COL_IS_CALL]
                    + main_trace.values[row_start + COL_IS_RET]
                    + main_trace.values[row_start + COL_IS_LOAD]
                    + main_trace.values[row_start + COL_IS_STORE]
                    + main_trace.values[row_start + COL_IS_PUSH]
                    + main_trace.values[row_start + COL_IS_POP]
                    + main_trace.values[row_start + COL_IS_ASSERT]
                    + main_trace.values[row_start + COL_IS_LOG]
                    + main_trace.values[row_start + COL_IS_SREAD]
                    + main_trace.values[row_start + COL_IS_SWRITE]
                    + main_trace.values[row_start + COL_IS_POSEIDON]
                    + main_trace.values[row_start + COL_IS_SYSCALL]
                    + main_trace.values[row_start + COL_IS_VERIFY_MERKLE]
                    + main_trace.values[row_start + COL_IS_HALT];
                let is_cpu_field = MyExtensionField::from(is_cpu);
                let r_active_field = MyExtensionField::from(main_trace.values[row_start + COL_REG_ACTIVE]);
                let m_active_field = MyExtensionField::from(main_trace.values[row_start + COL_MEM_ACTIVE]);
                
                let table_reg = Goldilocks::ZERO;
                let c_rs1 = register_term(alpha, beta, table_reg, main_trace.values[row_start + COL_CLK], main_trace.values[row_start + COL_RS1_IDX], main_trace.values[row_start + COL_RS1_VAL], Goldilocks::ZERO);
                let c_rs2 = register_term(alpha, beta, table_reg, main_trace.values[row_start + COL_CLK], main_trace.values[row_start + COL_RS2_IDX], main_trace.values[row_start + COL_RS2_VAL], Goldilocks::ZERO);
                let c_rd = register_term(alpha, beta, table_reg, main_trace.values[row_start + COL_CLK], main_trace.values[row_start + COL_RD_IDX], main_trace.values[row_start + COL_RD_VAL_NEW], Goldilocks::ONE);
                let c_reg = register_term(alpha, beta, table_reg, main_trace.values[row_start + COL_REG_CLK], main_trace.values[row_start + COL_REG_IDX], main_trace.values[row_start + COL_REG_VAL], main_trace.values[row_start + COL_REG_IS_WRITE]);

                let mut sum = aux_values[i * 2];
                sum += is_cpu_field * (gamma - c_rs1).inverse();
                sum += is_cpu_field * (gamma - c_rs2).inverse();
                sum += is_cpu_field * (gamma - c_rd).inverse();
                sum -= r_active_field * (gamma - c_reg).inverse();

                aux_values[(i + 1) * 2] = sum;

                let is_load = main_trace.values[row_start + COL_IS_LOAD];
                let is_store = main_trace.values[row_start + COL_IS_STORE];
                let is_mem_op = is_load + is_store;
                
                let cpu_mem_addr = main_trace.values[row_start + COL_RS1_VAL] + main_trace.values[row_start + COL_IMM];
                let cpu_mem_val = is_load * main_trace.values[row_start + COL_RD_VAL_NEW] + is_store * main_trace.values[row_start + COL_RS2_VAL];
                
                let table_mem = Goldilocks::ONE;
                let c_cpu_mem = register_term(alpha, beta, table_mem, main_trace.values[row_start + COL_CLK], cpu_mem_addr, cpu_mem_val, is_store);
                let c_mem = register_term(alpha, beta, table_mem, main_trace.values[row_start + COL_MEM_CLK], main_trace.values[row_start + COL_MEM_ADDR], main_trace.values[row_start + COL_MEM_VAL], main_trace.values[row_start + COL_MEM_IS_WRITE]);

                let mut sum_mem = aux_values[i * 2 + 1];
                sum_mem += MyExtensionField::from(is_mem_op) * (gamma - c_cpu_mem).inverse();
                sum_mem -= m_active_field * (gamma - c_mem).inverse();
                
                aux_values[(i + 1) * 2 + 1] = sum_mem;
            }

            RowMajorMatrix::new(aux_values, 2).flatten_to_base()
        },
    )
}

impl ProverAdapter for Plonky3Adapter {
    fn prove(trace: &[Step], num_steps: usize) -> Proof {
        let (matrix, trace_len) = trace_matrix(trace);
        let config = build_config();
        let air = BudAir { num_steps };
        let aux_matrix = matrix.clone();
        let proof = prove(
            &config,
            &air,
            matrix,
            Some(aux_trace_generator(aux_matrix, trace_len)),
            &vec![],
        );
        let data = bincode::serialize(&proof).expect("failed to serialize Plonky3 proof");

        Proof { data }
    }

    fn verify(proof: &Proof, num_steps: usize) -> bool {
        let config = build_config();
        let air = BudAir { num_steps };

        bincode::deserialize::<crate::bud_stark::Proof<MyConfig>>(&proof.data)
            .is_ok_and(|p3_proof| stark_verify(&config, &air, &p3_proof, &vec![]).is_ok())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bud_isa::{Instruction, Opcode};
    use bud_vm::Vm;

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

    fn prove_and_verify(program: Vec<u64>, setup: impl FnOnce(&mut Vm)) -> Proof {
        let mut vm = Vm::new(64);
        setup(&mut vm);
        vm.run(&program);

        let proof = Plonky3Adapter::prove(&vm.trace, vm.trace.len());
        assert!(!proof.data.is_empty());
        assert!(Plonky3Adapter::verify(&proof, vm.trace.len()));
        proof
    }

    #[test]
    fn proves_simple_add_trace() {
        let program = vec![
            inst(Opcode::Add, 1, 2, 3, 0),
            inst(Opcode::Halt, 0, 0, 0, 0),
        ];

        prove_and_verify(program, |vm| {
            vm.registers[2] = 10;
            vm.registers[3] = 20;
        });
    }

    #[test]
    fn proves_arithmetic_trace() {
        let program = vec![
            inst(Opcode::Add, 1, 2, 3, 0),
            inst(Opcode::Sub, 4, 1, 3, 0),
            inst(Opcode::Mul, 5, 4, 2, 0),
            inst(Opcode::Halt, 0, 0, 0, 0),
        ];

        prove_and_verify(program, |vm| {
            vm.registers[2] = 7;
            vm.registers[3] = 5;
        });
    }

    #[test]
    fn proves_load_immediate_trace() {
        let program = vec![
            inst(Opcode::Load, 1, 0, 0, 42),
            inst(Opcode::Halt, 0, 0, 0, 0),
        ];

        prove_and_verify(program, |_| {});
    }

    #[test]
    fn proof_bytes_roundtrip_before_verification() {
        let program = vec![
            inst(Opcode::Add, 1, 2, 3, 0),
            inst(Opcode::Halt, 0, 0, 0, 0),
        ];
        let proof = prove_and_verify(program, |vm| {
            vm.registers[2] = 3;
            vm.registers[3] = 4;
        });

        let p3_proof: crate::bud_stark::Proof<MyConfig> =
            bincode::deserialize(&proof.data).expect("proof bytes should decode");
        let encoded = bincode::serialize(&p3_proof).expect("proof should re-encode");
        let decoded = Proof { data: encoded };

        assert!(Plonky3Adapter::verify(&decoded, 2));
    }

    #[test]
    fn rejects_invalid_proof_bytes() {
        let proof = Proof {
            data: vec![1, 2, 3, 4],
        };

        assert!(!Plonky3Adapter::verify(&proof, 0));
    }
}
