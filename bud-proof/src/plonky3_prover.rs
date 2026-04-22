use crate::adapter::{Proof, ProverAdapter};
use crate::plonky3_air::*;
use bud_vm::Step;
use p3_matrix::dense::RowMajorMatrix;
use p3_goldilocks::Goldilocks;
use p3_field::PrimeField64;
use p3_uni_stark::{prove, verify, StarkConfig};
use p3_keccak::Keccak256Hash;
use p3_symmetric::{CompressionFunctionFromHasher, SerializingHasher};
use p3_merkle_tree::MerkleTreeMmcs;
use p3_challenger::{HashChallenger, SerializingChallenger64};
use p3_fri::{TwoAdicFriPcs, create_test_fri_params};
use p3_commit::ExtensionMmcs;
use p3_field::extension::BinomialExtensionField;
use p3_dft::Radix2DitParallel;

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

pub struct Plonky3Adapter;

impl ProverAdapter for Plonky3Adapter {
    fn prove(trace: &[Step], num_steps: usize) -> Proof {
        let mut events = Vec::new();
        for i in 0..32 {
            events.push(RegEvent { clk: 0, idx: i, val: 0, is_write: true, sub_clk: 0 });
        }
        for (i, step) in trace.iter().enumerate() {
            let clk = i as u64;
            events.push(RegEvent { clk, idx: step.src1_idx as u64, val: step.src1_val, is_write: false, sub_clk: 1 });
            events.push(RegEvent { clk, idx: step.src2_idx as u64, val: step.src2_val, is_write: false, sub_clk: 2 });
            events.push(RegEvent { clk, idx: step.dst_idx as u64, val: step.dst_val, is_write: true, sub_clk: 3 });
        }
        events.sort_by_key(|e| (e.idx, e.clk, e.sub_clk));

        let n_cpu = trace.len();
        let n_reg = events.len();
        let mut num_rows = n_cpu.max(n_reg).next_power_of_two();
        if num_rows < 16 { num_rows = 16; }

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
                0x0A => values[row_start + COL_IS_EQ]  = Goldilocks::new(1),
                0x0C => values[row_start + COL_IS_LT]  = Goldilocks::new(1),
                0x10 => values[row_start + COL_IS_JMP] = Goldilocks::new(1),
                0x11 => {
                    values[row_start + COL_IS_JNZ] = Goldilocks::new(1);
                    values[row_start + COL_JNZ_COND] = if step.src1_val != 0 { Goldilocks::new(1) } else { Goldilocks::new(0) };
                }
                0x14 => values[row_start + COL_IS_LOAD] = Goldilocks::new(1),
                0x00 => values[row_start + COL_IS_HALT] = Goldilocks::new(1),
                0x18 => values[row_start + COL_IS_ASSERT] = Goldilocks::new(1),
                0x1A => values[row_start + COL_IS_LOG] = Goldilocks::new(1),
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
            values[row_start + COL_REG_IS_WRITE] = if e.is_write { Goldilocks::new(1) } else { Goldilocks::new(0) };
            values[row_start + COL_REG_ACTIVE] = Goldilocks::new(1);
            
            if i < n_reg - 1 {
                if events[i+1].idx == e.idx {
                    values[row_start + COL_REG_SAME] = Goldilocks::new(1);
                }
            }
        }

        let matrix = RowMajorMatrix::new(values, TRACE_WIDTH);
        
        let hash = MyHasher::new(Keccak256Hash {});
        let compress = MyCompress::new(Keccak256Hash {});
        let val_mmcs = MyMmcs::new(hash, compress, 0);
        let challenge_mmcs = MyChallengeMmcs::new(val_mmcs.clone());
        let fri_params = create_test_fri_params(challenge_mmcs, 0);
        let inner_challenger = HashChallenger::<u8, Keccak256Hash, 32>::new(vec![], Keccak256Hash {});
        let challenger = MyChallenger::new(inner_challenger);
        let dft = Radix2DitParallel::default();
        let pcs = MyPcs::new(dft, val_mmcs, fri_params);
        let config = MyConfig::new(pcs, challenger);
        
        let air = BudAir { num_steps };
        let proof = prove(&config, &air, matrix, &vec![]);
        let data = bincode::serialize(&proof).unwrap_or(vec![]);
        Proof { data }
    }

    fn verify(proof: &Proof, num_steps: usize) -> bool {
        let hash = MyHasher::new(Keccak256Hash {});
        let compress = MyCompress::new(Keccak256Hash {});
        let val_mmcs = MyMmcs::new(hash, compress, 0);
        let challenge_mmcs = MyChallengeMmcs::new(val_mmcs.clone());
        let fri_params = create_test_fri_params(challenge_mmcs, 0);
        let inner_challenger = HashChallenger::<u8, Keccak256Hash, 32>::new(vec![], Keccak256Hash {});
        let challenger = MyChallenger::new(inner_challenger);
        let dft = Radix2DitParallel::default();
        let pcs = MyPcs::new(dft, val_mmcs, fri_params);
        let config = MyConfig::new(pcs, challenger);
        let air = BudAir { num_steps };
        if let Ok(p3_proof) = bincode::deserialize::<p3_uni_stark::Proof<MyConfig>>(&proof.data) {
            verify(&config, &air, &p3_proof, &vec![]).is_ok()
        } else {
            false
        }
    }
}
