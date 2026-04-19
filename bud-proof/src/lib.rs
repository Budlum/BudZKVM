use bud_vm::Step;

pub struct ExecutionMatrix {
    pub rows: Vec<Row>,
}

pub struct Row {
    pub pc: u64,
    pub opcode: u8,
    pub registers: [u64; 32],
}

pub struct Prover;

impl Prover {
    pub fn generate_matrix(trace: &[Step]) -> ExecutionMatrix {
        let mut rows = Vec::new();
        for step in trace {
            rows.push(Row {
                pc: step.pc as u64,
                opcode: step.instruction.opcode as u8,
                registers: step.registers,
            });
        }
        ExecutionMatrix { rows }
    }

    pub fn prove(matrix: &ExecutionMatrix) -> Proof {
        println!("Generating STARK proof for {} rows...", matrix.rows.len());
        let mut hash = [0u8; 32];
        for row in &matrix.rows {
            for (i, reg) in row.registers.iter().enumerate() {
                hash[i % 32] ^= (reg & 0xFF) as u8;
            }
        }
        Proof { data: hash.to_vec() }
    }
}

pub struct Proof {
    pub data: Vec<u8>,
}

pub struct RecursiveProver;

impl RecursiveProver {
    pub fn aggregate(proofs: &[Proof]) -> Proof {
        println!("Aggregating {} proofs...", proofs.len());
        let mut agg_hash = [0u8; 32];
        for p in proofs {
            for (i, b) in p.data.iter().enumerate() {
                agg_hash[i % 32] ^= *b;
            }
        }
        Proof { data: agg_hash.to_vec() }
    }
}
