use crate::adapter::{
    ExecutionPublicInputs, ProofEnvelope, ProverAdapter, ProverError, VerifyError,
};
use crate::bud_stark::{
    prove_with_preprocessed, setup_preprocessed,
    verify_with_preprocessed as stark_verify_with_preprocessed, StarkConfig,
};
use crate::plonky3_air::*;
use bincode::Options;
use bud_vm::{Step, Vm};
use p3_challenger::{HashChallenger, SerializingChallenger64};
use p3_commit::ExtensionMmcs;
use p3_dft::Radix2DitParallel;
use p3_field::extension::BinomialExtensionField;
use p3_field::{Field, PrimeCharacteristicRing};
use p3_fri::TwoAdicFriPcs;
use p3_goldilocks::Goldilocks;
use p3_keccak::Keccak256Hash;
use p3_matrix::dense::RowMajorMatrix;
use p3_matrix::Matrix;
use p3_merkle_tree::MerkleTreeMmcs;
use p3_symmetric::{CompressionFunctionFromHasher, SerializingHasher};
use p3_util::log2_strict_usize;
use std::boxed::Box;
use tiny_keccak::{Hasher, Keccak};

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

const STACK_BASE: u64 = 1 << 60;

pub struct Plonky3Adapter;

fn build_config() -> MyConfig {
    let hash = MyHasher::new(Keccak256Hash {});
    let compress = MyCompress::new(Keccak256Hash {});
    let val_mmcs = MyMmcs::new(hash, compress, 0);
    let challenge_mmcs = MyChallengeMmcs::new(val_mmcs.clone());
    let fri_params = p3_fri::FriParameters {
        log_blowup: 3,
        max_log_arity: 2,
        log_final_poly_len: 0,
        num_queries: 100,
        commit_proof_of_work_bits: 16,
        query_proof_of_work_bits: 16,
        mmcs: challenge_mmcs,
    };
    let inner_challenger = HashChallenger::<u8, Keccak256Hash, 32>::new(vec![], Keccak256Hash {});
    let challenger = MyChallenger::new(inner_challenger);
    let dft = Radix2DitParallel::default();
    let pcs = MyPcs::new(dft, val_mmcs, fri_params);
    MyConfig::new(pcs, challenger)
}

fn register_events(trace: &[Step]) -> Vec<RegEvent> {
    let mut events = Vec::new();

    for (i, step) in trace.iter().enumerate() {
        if step.instruction.opcode == bud_isa::Opcode::Halt {
            continue;
        }
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
            val: if step.dst_idx == 0 { 0 } else { step.dst_val },
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
        let clk = i as u64;
        if let Some(addr) = step.memory_addr {
            events.push(MemEvent {
                clk,
                addr: addr as u64,
                val: step.memory_val.unwrap_or(0),
                is_write: step.is_memory_write,
            });
        }

        let opcode = step.instruction.opcode;
        match opcode {
            bud_isa::Opcode::Push => {
                events.push(MemEvent {
                    clk,
                    addr: STACK_BASE + step.stack_pointer as u64 - 1,
                    val: step.src1_val,
                    is_write: true,
                });
            }
            bud_isa::Opcode::Pop => {
                events.push(MemEvent {
                    clk,
                    addr: STACK_BASE + step.stack_pointer as u64,
                    val: step.dst_val,
                    is_write: false,
                });
            }
            bud_isa::Opcode::Call => {
                events.push(MemEvent {
                    clk,
                    addr: STACK_BASE + step.stack_pointer as u64 - 1,
                    val: step.pc as u64 + 1,
                    is_write: true,
                });
            }
            bud_isa::Opcode::Ret => {
                events.push(MemEvent {
                    clk,
                    addr: STACK_BASE + step.stack_pointer as u64,
                    val: step.dst_val,
                    is_write: false,
                });
            }
            _ => {}
        }
    }
    events.sort_by_key(|e| (e.addr, e.clk));
    events
}

fn trace_matrix(trace: &[Step], _program: &[u64]) -> (RowMajorMatrix<Goldilocks>, usize) {
    let events = register_events(trace);
    let mem_events = memory_events(trace);
    let n_cpu = trace.len();
    let n_reg = events.len();
    let n_mem = mem_events.len();
    let num_rows = (3 * n_cpu + 1).next_power_of_two().max(16);

    let mut values = vec![Goldilocks::new(0); num_rows * TRACE_WIDTH];

    let mut running_gas = 0u64;

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
        values[row_start + COL_RD_VAL_NEW] = if step.dst_idx == 0 {
            Goldilocks::new(0)
        } else {
            Goldilocks::new(step.dst_val)
        };
        values[row_start + COL_NEXT_PC] = Goldilocks::new(step.next_pc as u64);
        values[row_start + COL_CPU_ACTIVE] = Goldilocks::new(1);

        let opcode = step.instruction.opcode;
        let cur_stack_ptr = match opcode {
            bud_isa::Opcode::Push | bud_isa::Opcode::Call => step.stack_pointer - 1,
            bud_isa::Opcode::Pop | bud_isa::Opcode::Ret => step.stack_pointer + 1,
            _ => step.stack_pointer,
        };
        values[row_start + COL_STACK_PTR] = Goldilocks::new(cur_stack_ptr as u64);

        let imm = step.instruction.imm;
        values[row_start + COL_IMM] = if imm < 0 {
            Goldilocks::new(0) - Goldilocks::new((-imm) as u64)
        } else {
            Goldilocks::new(imm as u64)
        };

        // Soundness & public input columns
        values[row_start + COL_GAS_USED] = Goldilocks::new(running_gas);
        running_gas = running_gas.saturating_add(Vm::gas_cost(opcode));

        values[row_start + COL_RAW_INST] = Goldilocks::new(step.instruction.encode());

        if opcode == bud_isa::Opcode::Div {
            let b = step.src2_val;
            let (inv, zero) = if b != 0 {
                (bud_vm::field_inverse_goldilocks(b), 0)
            } else {
                (0, 1)
            };
            values[row_start + COL_DIV_INV] = Goldilocks::new(inv);
            values[row_start + COL_DIV_ZERO] = Goldilocks::new(zero);
        }

        if opcode == bud_isa::Opcode::Inv {
            let a = step.src1_val;
            let zero = if a != 0 { 0 } else { 1 };
            values[row_start + COL_INV_ZERO] = Goldilocks::new(zero);
        }

        if opcode == bud_isa::Opcode::Eq || opcode == bud_isa::Opcode::Neq {
            let diff = step.src1_val.wrapping_sub(step.src2_val);
            let inv = if diff != 0 {
                bud_vm::field_inverse_goldilocks(diff)
            } else {
                0
            };
            values[row_start + COL_EQ_DIFF_INV] = Goldilocks::new(inv);
        }

        if opcode == bud_isa::Opcode::Jnz {
            let cond = step.src1_val;
            let inv = if cond != 0 {
                bud_vm::field_inverse_goldilocks(cond)
            } else {
                0
            };
            values[row_start + COL_JNZ_COND_INV] = Goldilocks::new(inv);
        }

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
            values[row_start + COL_STACK_PTR] =
                Goldilocks::new(trace[n_cpu - 1].stack_pointer as u64);
        }
        values[row_start + COL_GAS_USED] = Goldilocks::new(running_gas);
        values[row_start + COL_RAW_INST] = Goldilocks::new(
            bud_isa::Instruction {
                opcode: bud_isa::Opcode::Halt,
                rd: 0,
                rs1: 0,
                rs2: 0,
                imm: 0,
            }
            .encode(),
        );
        values[row_start + COL_CPU_ACTIVE] = Goldilocks::new(0);
    }

    for (i, e) in events.iter().enumerate() {
        let row_start = i * TRACE_WIDTH;
        values[row_start + COL_REG_CLK] = Goldilocks::new(e.clk);
        values[row_start + COL_REG_IDX] = Goldilocks::new(e.idx);
        values[row_start + COL_REG_VAL] = Goldilocks::new(e.val);
        values[row_start + COL_REG_SUB_CLK] = Goldilocks::new(e.sub_clk as u64);
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

    (RowMajorMatrix::new(values, TRACE_WIDTH), n_cpu)
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

#[allow(clippy::type_complexity)]
fn aux_trace_generator(
    main_trace: RowMajorMatrix<Goldilocks>,
    trace_len: usize,
    program: Vec<u64>,
) -> Box<dyn FnOnce(&[MyExtensionField]) -> RowMajorMatrix<Goldilocks>> {
    Box::new(move |random_challenges| {
        let num_rows = main_trace.height();
        let mut aux_values = vec![MyExtensionField::ZERO; num_rows * 3]; // Register, Memory, Program LogUp
        let alpha = random_challenges[0];
        let beta = random_challenges[1];
        let gamma = random_challenges[2];

        let b2 = beta * beta;

        let mut s_reg = MyExtensionField::ZERO;
        let mut s_mem = MyExtensionField::ZERO;
        let mut s_prog = MyExtensionField::ZERO;

        aux_values[0] = s_reg;
        aux_values[1] = s_mem;
        aux_values[2] = s_prog;

        for i in 0..num_rows - 1 {
            let row_start = i * TRACE_WIDTH;
            let row = &main_trace.values[row_start..row_start + TRACE_WIDTH];

            // Register LogUp
            let is_add = row[COL_IS_ADD];
            let is_sub = row[COL_IS_SUB];
            let is_mul = row[COL_IS_MUL];
            let is_div = row[COL_IS_DIV];
            let is_inv = row[COL_IS_INV];
            let is_and = row[COL_IS_AND];
            let is_or = row[COL_IS_OR];
            let is_xor = row[COL_IS_XOR];
            let is_not = row[COL_IS_NOT];
            let is_eq = row[COL_IS_EQ];
            let is_neq = row[COL_IS_NEQ];
            let is_lt = row[COL_IS_LT];
            let is_gt = row[COL_IS_GT];
            let is_lte = row[COL_IS_LTE];
            let is_gte = row[COL_IS_GTE];
            let is_jmp = row[COL_IS_JMP];
            let is_jnz = row[COL_IS_JNZ];
            let is_call = row[COL_IS_CALL];
            let is_ret = row[COL_IS_RET];
            let is_load = row[COL_IS_LOAD];
            let is_store = row[COL_IS_STORE];
            let is_push = row[COL_IS_PUSH];
            let is_pop = row[COL_IS_POP];
            let is_assert = row[COL_IS_ASSERT];
            let is_log = row[COL_IS_LOG];
            let is_sread = row[COL_IS_SREAD];
            let is_swrite = row[COL_IS_SWRITE];
            let is_poseidon = row[COL_IS_POSEIDON];
            let is_syscall = row[COL_IS_SYSCALL];
            let is_verify_merkle = row[COL_IS_VERIFY_MERKLE];

            let is_real_op = is_add
                + is_sub
                + is_mul
                + is_div
                + is_inv
                + is_and
                + is_or
                + is_xor
                + is_not
                + is_eq
                + is_neq
                + is_lt
                + is_gt
                + is_lte
                + is_gte
                + is_jmp
                + is_jnz
                + is_call
                + is_ret
                + is_load
                + is_store
                + is_push
                + is_pop
                + is_assert
                + is_log
                + is_sread
                + is_swrite
                + is_poseidon
                + is_syscall
                + is_verify_merkle;

            let clk = row[COL_CLK];
            let pc = row[COL_PC];
            let rs1_idx = row[COL_RS1_IDX];
            let rs2_idx = row[COL_RS2_IDX];
            let rd_idx = row[COL_RD_IDX];
            let rs1_val = row[COL_RS1_VAL];
            let rs2_val = row[COL_RS2_VAL];
            let rd_val_new = row[COL_RD_VAL_NEW];

            let reg_active = row[COL_REG_ACTIVE];
            let reg_clk = row[COL_REG_CLK];
            let reg_sub_clk = row[COL_REG_SUB_CLK];
            let reg_idx = row[COL_REG_IDX];
            let reg_val = row[COL_REG_VAL];
            let reg_is_write = row[COL_REG_IS_WRITE];

            let clk_rs1 = clk * Goldilocks::from_u64(4) + Goldilocks::from_u64(1);
            let clk_rs2 = clk * Goldilocks::from_u64(4) + Goldilocks::from_u64(2);
            let clk_rd = clk * Goldilocks::from_u64(4) + Goldilocks::from_u64(3);
            let clk_reg = reg_clk * Goldilocks::from_u64(4) + reg_sub_clk;

            let c_rs1 = register_term(
                alpha,
                beta,
                Goldilocks::ZERO,
                clk_rs1,
                rs1_idx,
                rs1_val,
                Goldilocks::ZERO,
            );
            let c_rs2 = register_term(
                alpha,
                beta,
                Goldilocks::ZERO,
                clk_rs2,
                rs2_idx,
                rs2_val,
                Goldilocks::ZERO,
            );
            let c_rd = register_term(
                alpha,
                beta,
                Goldilocks::ZERO,
                clk_rd,
                rd_idx,
                rd_val_new,
                Goldilocks::ONE,
            );
            let c_reg = register_term(
                alpha,
                beta,
                Goldilocks::ZERO,
                clk_reg,
                reg_idx,
                reg_val,
                reg_is_write,
            );

            if is_real_op != Goldilocks::ZERO {
                s_reg += (gamma - c_rs1).inverse()
                    + (gamma - c_rs2).inverse()
                    + (gamma - c_rd).inverse();
            }
            if reg_active != Goldilocks::ZERO {
                s_reg -= (gamma - c_reg).inverse();
            }

            // Memory LogUp
            let m_active = row[COL_MEM_ACTIVE];
            let m_clk = row[COL_MEM_CLK];
            let m_addr = row[COL_MEM_ADDR];
            let m_val = row[COL_MEM_VAL];
            let m_is_write = row[COL_MEM_IS_WRITE];

            let is_real_mem_op = (is_load + is_store)
                * if rs1_idx != Goldilocks::ZERO {
                    Goldilocks::ONE
                } else {
                    Goldilocks::ZERO
                };
            let is_stack_op = is_push + is_pop + is_call + is_ret;
            let is_any_mem_op = is_real_mem_op + is_stack_op;

            let stack_ptr = row[COL_STACK_PTR];
            let stack_base = Goldilocks::from_u64(STACK_BASE);
            let stack_addr = stack_base
                + (is_push + is_call) * stack_ptr
                + (is_pop + is_ret) * (stack_ptr - Goldilocks::ONE);

            let final_mem_addr =
                is_real_mem_op * (row[COL_RS1_VAL] + row[COL_IMM]) + is_stack_op * stack_addr;

            let is_write = is_store + is_push + is_call;
            let cpu_mem_val = is_load * row[COL_RD_VAL_NEW]
                + is_store * row[COL_RS2_VAL]
                + is_push * row[COL_RS1_VAL]
                + is_pop * row[COL_RD_VAL_NEW]
                + is_call * (row[COL_PC] + Goldilocks::ONE)
                + is_ret * row[COL_NEXT_PC];

            let c_cpu_mem = register_term(
                alpha,
                beta,
                Goldilocks::ONE,
                clk,
                final_mem_addr,
                cpu_mem_val,
                is_write,
            );
            let c_mem = register_term(
                alpha,
                beta,
                Goldilocks::ONE,
                m_clk,
                m_addr,
                m_val,
                m_is_write,
            );

            if is_any_mem_op != Goldilocks::ZERO {
                s_mem += (gamma - c_cpu_mem).inverse();
            }
            if m_active != Goldilocks::ZERO {
                s_mem -= (gamma - c_mem).inverse();
            }

            // Program LogUp
            let raw_inst = row[COL_RAW_INST];
            let term_cpu_prog =
                alpha + beta * MyExtensionField::from(pc) + b2 * MyExtensionField::from(raw_inst);

            let pre_pc = Goldilocks::from_u64(i as u64);
            let pre_inst = Goldilocks::from_u64(program.get(i).copied().unwrap_or(0));
            let term_pre_prog = alpha
                + beta * MyExtensionField::from(pre_pc)
                + b2 * MyExtensionField::from(pre_inst);

            let diff_cpu_prog = gamma - term_cpu_prog;
            let diff_pre_prog = gamma - term_pre_prog;

            if i < trace_len {
                s_prog += diff_cpu_prog.inverse();
            }
            if i < program.len() {
                s_prog -= diff_pre_prog.inverse();
            }

            aux_values[(i + 1) * 3] = s_reg;
            aux_values[(i + 1) * 3 + 1] = s_mem;
            aux_values[(i + 1) * 3 + 2] = s_prog;
        }

        RowMajorMatrix::new(aux_values, 3).flatten_to_base()
    })
}

fn to_public_values(pi: &ExecutionPublicInputs) -> Vec<Goldilocks> {
    let mut vals = Vec::new();

    vals.push(Goldilocks::from_u64(pi.chain_id & 0xFFFF_FFFF));
    vals.push(Goldilocks::from_u64(pi.chain_id >> 32));

    for chunk in pi.program_hash.chunks_exact(4) {
        let val = u32::from_le_bytes(chunk.try_into().unwrap());
        vals.push(Goldilocks::from_u64(val as u64));
    }

    for chunk in pi.initial_state_root.chunks_exact(4) {
        let val = u32::from_le_bytes(chunk.try_into().unwrap());
        vals.push(Goldilocks::from_u64(val as u64));
    }

    for chunk in pi.final_state_root.chunks_exact(4) {
        let val = u32::from_le_bytes(chunk.try_into().unwrap());
        vals.push(Goldilocks::from_u64(val as u64));
    }

    vals.push(Goldilocks::from_u64(pi.sender & 0xFFFF_FFFF));
    vals.push(Goldilocks::from_u64(pi.sender >> 32));

    vals.push(Goldilocks::from_u64(pi.nonce & 0xFFFF_FFFF));
    vals.push(Goldilocks::from_u64(pi.nonce >> 32));

    vals.push(Goldilocks::from_u64(pi.block_height & 0xFFFF_FFFF));
    vals.push(Goldilocks::from_u64(pi.block_height >> 32));

    vals.push(Goldilocks::from_u64(pi.gas_limit & 0xFFFF_FFFF));
    vals.push(Goldilocks::from_u64(pi.gas_limit >> 32));

    vals.push(Goldilocks::from_u64(pi.gas_used & 0xFFFF_FFFF));
    vals.push(Goldilocks::from_u64(pi.gas_used >> 32));

    vals.push(Goldilocks::from_u64(pi.exit_code & 0xFFFF_FFFF));
    vals.push(Goldilocks::from_u64(pi.exit_code >> 32));

    vals.push(Goldilocks::from_u64(pi.trace_len & 0xFFFF_FFFF));
    vals.push(Goldilocks::from_u64(pi.trace_len >> 32));

    for chunk in pi.event_digest.chunks_exact(4) {
        let val = u32::from_le_bytes(chunk.try_into().unwrap());
        vals.push(Goldilocks::from_u64(val as u64));
    }

    vals
}

impl ProverAdapter for Plonky3Adapter {
    fn prove(
        trace: &[Step],
        public_inputs: &ExecutionPublicInputs,
        program: &[u64],
    ) -> Result<ProofEnvelope, ProverError> {
        let (matrix, trace_len) = trace_matrix(trace, program);
        let config = build_config();

        let air = BudAir {
            num_steps: trace.len(),
            program: program.to_vec(),
        };

        let degree_bits = log2_strict_usize(matrix.height());
        let preprocessed = setup_preprocessed(&config, &air, degree_bits);
        let preprocessed_ref = preprocessed.as_ref().map(|(p, _)| p);

        let public_values = to_public_values(public_inputs);

        let p3_proof = prove_with_preprocessed(
            &config,
            &air,
            matrix.clone(),
            Some(aux_trace_generator(
                matrix.clone(),
                trace_len,
                program.to_vec(),
            )),
            &public_values,
            preprocessed_ref,
        );

        let options = bincode::options().with_limit(10 * 1024 * 1024);
        let proof_bytes = options
            .serialize(&p3_proof)
            .map_err(|e| ProverError::SerializationError(e.to_string()))?;

        Ok(ProofEnvelope {
            proof_format_version: 1,
            backend: "Plonky3-Keccak-Goldilocks".to_string(),
            p3_version: "0.5.2".to_string(),
            fri_params_id: "test_fri_params".to_string(),
            public_inputs_hash: public_inputs.hash(),
            proof_bytes,
            degree_bits: degree_bits as u32,
        })
    }

    fn verify(
        envelope: &ProofEnvelope,
        expected_inputs: &ExecutionPublicInputs,
        program: &[u64],
    ) -> Result<(), VerifyError> {
        if envelope.proof_format_version != 1 {
            return Err(VerifyError::InvalidEnvelope(
                "Unsupported proof format version".to_string(),
            ));
        }
        if envelope.backend != "Plonky3-Keccak-Goldilocks" {
            return Err(VerifyError::InvalidEnvelope(
                "Unsupported backend".to_string(),
            ));
        }
        if envelope.p3_version != "0.5.2" {
            return Err(VerifyError::InvalidEnvelope(
                "Unsupported Plonky3 version".to_string(),
            ));
        }
        if envelope.fri_params_id != "test_fri_params" {
            return Err(VerifyError::InvalidEnvelope(
                "Unsupported FRI parameters".to_string(),
            ));
        }
        if envelope.public_inputs_hash != expected_inputs.hash() {
            return Err(VerifyError::PublicInputsMismatch);
        }

        // Program hash verification
        let program_bytes: Vec<u8> = program
            .iter()
            .flat_map(|&inst| inst.to_le_bytes().to_vec())
            .collect();
        let mut hasher = Keccak::v256();
        hasher.update(&program_bytes);
        let mut computed_prog_hash = [0u8; 32];
        hasher.finalize(&mut computed_prog_hash);

        if computed_prog_hash != expected_inputs.program_hash {
            return Err(VerifyError::PublicInputsMismatch);
        }

        let config = build_config();
        let air = BudAir {
            num_steps: expected_inputs.trace_len as usize,
            program: program.to_vec(),
        };

        let degree_bits = log2_strict_usize(
            (3 * expected_inputs.trace_len as usize + 1)
                .next_power_of_two()
                .max(16),
        );
        let preprocessed = setup_preprocessed(&config, &air, degree_bits);
        let preprocessed_vk_ref = preprocessed.as_ref().map(|(_, vk)| vk);

        let public_values = to_public_values(expected_inputs);

        let options = bincode::options().with_limit(10 * 1024 * 1024);
        let p3_proof = options
            .deserialize::<crate::bud_stark::Proof<MyConfig>>(&envelope.proof_bytes)
            .map_err(|e| VerifyError::DeserializationError(e.to_string()))?;

        stark_verify_with_preprocessed(
            &config,
            &air,
            &p3_proof,
            &public_values,
            preprocessed_vk_ref,
        )
        .map_err(|_| VerifyError::InvalidProof)
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

    fn prove_and_verify(program: Vec<u64>, setup: impl FnOnce(&mut Vm)) -> ProofEnvelope {
        let mut vm = Vm::new(64);
        setup(&mut vm);
        let receipt = vm.run_receipt(&program);
        assert!(receipt.success);

        let initial_root = [0u8; 32];
        let final_root = [0u8; 32];

        let program_bytes: Vec<u8> = program
            .iter()
            .flat_map(|&inst| inst.to_le_bytes().to_vec())
            .collect();
        let mut hasher = Keccak::v256();
        hasher.update(&program_bytes);
        let mut program_hash = [0u8; 32];
        hasher.finalize(&mut program_hash);

        let pi = ExecutionPublicInputs {
            chain_id: 1,
            program_hash,
            initial_state_root: initial_root,
            final_state_root: final_root,
            sender: vm.context.sender,
            nonce: vm.context.nonce,
            block_height: vm.context.block_height,
            gas_limit: vm.gas_limit,
            gas_used: vm.gas_used,
            exit_code: 0,
            trace_len: vm.trace.len() as u64,
            event_digest: [0u8; 32],
        };

        let envelope = Plonky3Adapter::prove(&vm.trace, &pi, &program).unwrap();
        let verify_res = Plonky3Adapter::verify(&envelope, &pi, &program);
        if let Err(ref e) = verify_res {
            println!("Verification failed with error: {:?}", e);
        }
        assert!(verify_res.is_ok());
        envelope
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
    fn proves_push_pop_trace() {
        let program = vec![
            inst(Opcode::Load, 1, 0, 0, 123),
            inst(Opcode::Push, 0, 1, 0, 0),
            inst(Opcode::Pop, 2, 0, 0, 0),
            inst(Opcode::Halt, 0, 0, 0, 0),
        ];

        prove_and_verify(program, |_| {});
    }

    #[test]
    fn proves_call_ret_trace() {
        let program = vec![
            inst(Opcode::Call, 0, 0, 0, 2),
            inst(Opcode::Halt, 0, 0, 0, 0),
            inst(Opcode::Load, 1, 0, 0, 7),
            inst(Opcode::Ret, 0, 0, 0, 0),
        ];

        prove_and_verify(program, |_| {});
    }

    #[test]
    fn proves_nested_call_trace() {
        let program = vec![
            inst(Opcode::Call, 0, 0, 0, 4), // Call B
            inst(Opcode::Halt, 0, 0, 0, 0),
            // Func A (index 2)
            inst(Opcode::Load, 1, 0, 0, 42),
            inst(Opcode::Ret, 0, 0, 0, 0),
            // Func B (index 4)
            inst(Opcode::Call, 0, 0, 0, -2), // Call A
            inst(Opcode::Ret, 0, 0, 0, 0),
        ];

        prove_and_verify(program, |_| {});
    }

    #[test]
    fn rejects_invalid_proof_bytes() {
        let envelope = ProofEnvelope {
            proof_format_version: 1,
            backend: "Plonky3-Keccak-Goldilocks".to_string(),
            p3_version: "0.5.2".to_string(),
            fri_params_id: "test_fri_params".to_string(),
            public_inputs_hash: [0u8; 32],
            proof_bytes: vec![1, 2, 3, 4],
            degree_bits: 4,
        };

        let pi = ExecutionPublicInputs {
            chain_id: 1,
            program_hash: [0u8; 32],
            initial_state_root: [0u8; 32],
            final_state_root: [0u8; 32],
            sender: 0,
            nonce: 0,
            block_height: 0,
            gas_limit: 1000000,
            gas_used: 0,
            exit_code: 0,
            trace_len: 0,
            event_digest: [0u8; 32],
        };

        let res = Plonky3Adapter::verify(&envelope, &pi, &[]);
        assert!(res.is_err());
    }

    #[test]
    fn rejects_tampered_public_inputs() {
        let program = vec![
            inst(Opcode::Load, 1, 0, 0, 42),
            inst(Opcode::Halt, 0, 0, 0, 0),
        ];

        let mut vm = Vm::new(64);
        let receipt = vm.run_receipt(&program);
        assert!(receipt.success);

        let initial_root = [0u8; 32];
        let final_root = [0u8; 32];
        let pi = ExecutionPublicInputs {
            chain_id: 1,
            program_hash: [0u8; 32],
            initial_state_root: initial_root,
            final_state_root: final_root,
            sender: 100, // Expected sender
            nonce: 5,
            block_height: 10,
            gas_limit: vm.gas_limit,
            gas_used: vm.gas_used,
            exit_code: 0,
            trace_len: vm.trace.len() as u64,
            event_digest: [0u8; 32],
        };

        // Prover generates valid proof
        let envelope = Plonky3Adapter::prove(&vm.trace, &pi, &program).unwrap();

        // Verifier uses tampered public inputs (e.g. different sender)
        let mut tampered_pi = pi.clone();
        tampered_pi.sender = 999;
        assert!(matches!(
            Plonky3Adapter::verify(&envelope, &tampered_pi, &program),
            Err(VerifyError::PublicInputsMismatch)
        ));

        // Verifier uses different gas_used
        let mut tampered_pi = pi.clone();
        tampered_pi.gas_used = 12345;
        // This will mismatch the public input hash
        assert!(matches!(
            Plonky3Adapter::verify(&envelope, &tampered_pi, &program),
            Err(VerifyError::PublicInputsMismatch)
        ));
    }

    #[test]
    fn rejects_tampered_program() {
        let program = vec![
            inst(Opcode::Load, 1, 0, 0, 42),
            inst(Opcode::Halt, 0, 0, 0, 0),
        ];

        let mut vm = Vm::new(64);
        let receipt = vm.run_receipt(&program);
        assert!(receipt.success);

        let initial_root = [0u8; 32];
        let final_root = [0u8; 32];
        let pi = ExecutionPublicInputs {
            chain_id: 1,
            program_hash: [0u8; 32],
            initial_state_root: initial_root,
            final_state_root: final_root,
            sender: 0,
            nonce: 0,
            block_height: 0,
            gas_limit: vm.gas_limit,
            gas_used: vm.gas_used,
            exit_code: 0,
            trace_len: vm.trace.len() as u64,
            event_digest: [0u8; 32],
        };

        let envelope = Plonky3Adapter::prove(&vm.trace, &pi, &program).unwrap();

        // Verifier attempts to verify with a different program
        let tampered_program = vec![
            inst(Opcode::Load, 1, 0, 0, 999), // Different loaded value
            inst(Opcode::Halt, 0, 0, 0, 0),
        ];

        let res = Plonky3Adapter::verify(&envelope, &pi, &tampered_program);
        assert!(res.is_err());
    }
}
