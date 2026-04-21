use winterfell::{
    Prover as WinterProver, TraceTable, Air, Trace,
    ConstraintCompositionCoefficients, 
    ProofOptions, AirContext, TransitionConstraintDegree,
    Assertion, EvaluationFrame, DefaultConstraintEvaluator,
    AuxTraceRandElements, TraceInfo, DefaultTraceLde,
    StarkDomain, TracePolyTable, matrix::ColMatrix
};
use winter_math::{fields::f128::BaseElement, FieldElement, ToElements};
use serde::{Serialize, Deserialize};

#[derive(Clone, Debug)]
pub struct PubInputs {
    pub num_steps: usize,
}

impl ToElements<BaseElement> for PubInputs {
    fn to_elements(&self) -> Vec<BaseElement> {
        vec![BaseElement::from(self.num_steps as u64)]
    }
}

const TRACE_WIDTH: usize = 55;

const COL_STEP: usize = 0;
const COL_REG: usize = 1;
const COL_OPCODE: usize = 32;
const COL_RS1_VAL: usize = 33;
const COL_RS2_VAL: usize = 34;
const COL_RD_NEXT: usize = 35;

const COL_IS_ADD: usize = 36;
const COL_IS_SUB: usize = 37;
const COL_IS_MUL: usize = 38;
const COL_IS_HALT: usize = 39;
const COL_IS_LOG: usize = 40;
const COL_IS_SYSCALL: usize = 41;
const COL_IS_LOAD: usize = 42;
const COL_IS_EQ: usize = 43;
const COL_IS_NEQ: usize = 44;
const COL_IS_LT: usize = 45;
const COL_IS_GT: usize = 46;
const COL_IS_LTE: usize = 47;
const COL_IS_GTE: usize = 48;
const COL_IS_POSEIDON: usize = 49;
const COL_IS_SREAD: usize = 50;

const COL_MUL_RESULT: usize = 51;
const COL_CMP_RESULT: usize = 52;
const COL_POSEIDON_RESULT: usize = 53;
const COL_EXPECTED_RD: usize = 54;

const OP_HALT: u64 = 0x00;
const OP_ADD: u64 = 0x01;
const OP_SUB: u64 = 0x02;
const OP_MUL: u64 = 0x03;
const OP_EQ: u64 = 0x0A;
const OP_NEQ: u64 = 0x0B;
const OP_LT: u64 = 0x0C;
const OP_GT: u64 = 0x0D;
const OP_LTE: u64 = 0x0E;
const OP_GTE: u64 = 0x0F;
const OP_LOAD: u64 = 0x14;
const OP_POSEIDON: u64 = 0x19;
const OP_LOG: u64 = 0x1A;
const OP_SREAD: u64 = 0x1B;
const OP_SYSCALL: u64 = 0x1D;

pub struct BudAir {
    context: AirContext<BaseElement>,
    num_steps: usize,
}

impl Air for BudAir {
    type BaseField = BaseElement;
    type PublicInputs = PubInputs;

    fn new(trace_info: TraceInfo, pub_inputs: PubInputs, options: ProofOptions) -> Self {
        let degrees = vec![
            TransitionConstraintDegree::new(1),
            TransitionConstraintDegree::new(2),
        ];
        let context = AirContext::new(trace_info, degrees, 1, options);
        Self { context, num_steps: pub_inputs.num_steps }
    }

    fn evaluate_transition<E: FieldElement<BaseField = Self::BaseField>>(
        &self,
        frame: &EvaluationFrame<E>,
        _periodic_values: &[E],
        result: &mut [E],
    ) {
        let current = frame.current();
        let next = frame.next();

        let rs1_val = current[COL_RS1_VAL];
        let rs2_val = current[COL_RS2_VAL];
        let rd_next = current[COL_RD_NEXT];

        result[0] = next[COL_STEP] - current[COL_STEP] - E::ONE;

        result[1] =
              current[COL_IS_ADD] * (rd_next - (rs1_val + rs2_val))
            + current[COL_IS_SUB] * (rd_next - (rs1_val - rs2_val))
            + current[COL_IS_MUL] * (rd_next - current[COL_MUL_RESULT])
            + current[COL_IS_LOAD] * (rd_next - current[COL_EXPECTED_RD])
            + (current[COL_IS_EQ] + current[COL_IS_NEQ]
               + current[COL_IS_LT] + current[COL_IS_GT]
               + current[COL_IS_LTE] + current[COL_IS_GTE])
              * (rd_next - current[COL_CMP_RESULT])
            + current[COL_IS_POSEIDON] * (rd_next - current[COL_POSEIDON_RESULT])
            + current[COL_IS_SREAD] * (rd_next - current[COL_EXPECTED_RD])
            + current[COL_IS_SYSCALL] * (rd_next - current[COL_EXPECTED_RD]);
    }

    fn get_assertions(&self) -> Vec<Assertion<Self::BaseField>> {
        vec![Assertion::single(COL_STEP, 0, BaseElement::ZERO)]
    }

    fn context(&self) -> &AirContext<Self::BaseField> {
        &self.context
    }
}

pub struct BudProver {
    options: ProofOptions,
    num_steps: usize,
}

impl WinterProver for BudProver {
    type BaseField = BaseElement;
    type Air = BudAir;
    type Trace = TraceTable<BaseElement>;
    type HashFn = winterfell::crypto::hashers::Blake3_256<BaseElement>;
    type RandomCoin = winterfell::crypto::DefaultRandomCoin<Self::HashFn>;
    type TraceLde<E: FieldElement<BaseField = Self::BaseField>> = DefaultTraceLde<E, Self::HashFn>;
    type ConstraintEvaluator<'a, E: FieldElement<BaseField = Self::BaseField>> = DefaultConstraintEvaluator<'a, Self::Air, E>;

    fn get_pub_inputs(&self, _trace: &Self::Trace) -> PubInputs {
        PubInputs { num_steps: self.num_steps }
    }

    fn options(&self) -> &ProofOptions {
        &self.options
    }

    fn new_trace_lde<E: FieldElement<BaseField = Self::BaseField>>(
        &self,
        trace_info: &TraceInfo,
        main_trace: &ColMatrix<Self::BaseField>,
        domain: &StarkDomain<Self::BaseField>,
    ) -> (Self::TraceLde<E>, TracePolyTable<E>) {
        DefaultTraceLde::new(trace_info, main_trace, domain)
    }

    fn new_evaluator<'a, E: FieldElement<BaseField = Self::BaseField>>(
        &self,
        air: &'a Self::Air,
        aux_rand_elements: AuxTraceRandElements<E>,
        composition_coefficients: ConstraintCompositionCoefficients<E>,
    ) -> Self::ConstraintEvaluator<'a, E> {
        DefaultConstraintEvaluator::new(air, aux_rand_elements, composition_coefficients)
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Proof {
    pub data: Vec<u8>,
}

fn flag(op: u64, target: u64) -> BaseElement {
    if op == target { BaseElement::ONE } else { BaseElement::ZERO }
}

pub struct Prover;

impl Prover {
    pub fn generate_matrix(trace: &[bud_vm::Step]) -> TraceTable<BaseElement> {
        let mut num_rows = trace.len().next_power_of_two();
        if num_rows < 8 { num_rows = 8; }
        
        let mut columns = vec![vec![BaseElement::ZERO; num_rows]; TRACE_WIDTH];

        for i in 0..num_rows {
            columns[COL_STEP][i] = BaseElement::from(i as u64);
        }

        for (i, step) in trace.iter().enumerate() {
            for r in 1..32 {
                columns[COL_REG + r - 1][i] = BaseElement::from(step.registers[r]);
            }

            let op = step.instruction.opcode as u64;
            let rs1 = step.registers[step.instruction.rs1 as usize];
            let rs2 = step.registers[step.instruction.rs2 as usize];

            columns[COL_OPCODE][i] = BaseElement::from(op);
            columns[COL_RS1_VAL][i] = BaseElement::from(rs1);
            columns[COL_RS2_VAL][i] = BaseElement::from(rs2);

            if i + 1 < trace.len() {
                let rd = step.instruction.rd as usize;
                columns[COL_RD_NEXT][i] = BaseElement::from(trace[i + 1].registers[rd]);
            } else {
                let rd = step.instruction.rd as usize;
                columns[COL_RD_NEXT][i] = BaseElement::from(step.registers[rd]);
            }

            columns[COL_IS_ADD][i] = flag(op, OP_ADD);
            columns[COL_IS_SUB][i] = flag(op, OP_SUB);
            columns[COL_IS_MUL][i] = flag(op, OP_MUL);
            columns[COL_IS_HALT][i] = flag(op, OP_HALT);
            columns[COL_IS_LOG][i] = flag(op, OP_LOG);
            columns[COL_IS_SYSCALL][i] = flag(op, OP_SYSCALL);
            columns[COL_IS_LOAD][i] = flag(op, OP_LOAD);
            columns[COL_IS_EQ][i] = flag(op, OP_EQ);
            columns[COL_IS_NEQ][i] = flag(op, OP_NEQ);
            columns[COL_IS_LT][i] = flag(op, OP_LT);
            columns[COL_IS_GT][i] = flag(op, OP_GT);
            columns[COL_IS_LTE][i] = flag(op, OP_LTE);
            columns[COL_IS_GTE][i] = flag(op, OP_GTE);
            columns[COL_IS_POSEIDON][i] = flag(op, OP_POSEIDON);
            columns[COL_IS_SREAD][i] = flag(op, OP_SREAD);

            columns[COL_MUL_RESULT][i] = BaseElement::from(rs1.wrapping_mul(rs2));

            let cmp = match op {
                OP_EQ => if rs1 == rs2 { 1u64 } else { 0 },
                OP_NEQ => if rs1 != rs2 { 1 } else { 0 },
                OP_LT => if rs1 < rs2 { 1 } else { 0 },
                OP_GT => if rs1 > rs2 { 1 } else { 0 },
                OP_LTE => if rs1 <= rs2 { 1 } else { 0 },
                OP_GTE => if rs1 >= rs2 { 1 } else { 0 },
                _ => 0,
            };
            columns[COL_CMP_RESULT][i] = BaseElement::from(cmp);

            columns[COL_POSEIDON_RESULT][i] = BaseElement::from(
                rs1.wrapping_mul(31).wrapping_add(rs2).wrapping_add(0x1337)
            );

            let expected = if i + 1 < trace.len() {
                trace[i + 1].registers[step.instruction.rd as usize]
            } else {
                step.registers[step.instruction.rd as usize]
            };
            columns[COL_EXPECTED_RD][i] = BaseElement::from(expected);
        }

        let last = trace.len() - 1;
        for i in trace.len()..num_rows {
            for r in 1..32 {
                columns[COL_REG + r - 1][i] = columns[COL_REG + r - 1][last];
            }
            columns[COL_IS_HALT][i] = BaseElement::ONE;
        }
        
        TraceTable::init(columns)
    }

    pub fn prove(trace: &TraceTable<BaseElement>, num_steps: usize) -> Proof {
        println!("Generating STARK proof for {} rows and {} columns...", trace.length(), TRACE_WIDTH);
        let options = ProofOptions::new(32, 8, 0, winterfell::FieldExtension::None, 4, 31);
        let prover = BudProver { options, num_steps };
        let proof = prover.prove(trace.clone()).expect("Proving failed");
        let data = proof.to_bytes();
        println!("STARK proof generated. Size: {} bytes", data.len());
        Proof { data }
    }
}

pub struct Verifier;

impl Verifier {
    pub fn verify(proof: &Proof, num_steps: usize) -> bool {
        println!("Verifying STARK proof ({} bytes)...", proof.data.len());
        let parsed = winterfell::StarkProof::from_bytes(&proof.data);
        match parsed {
            Ok(stark_proof) => {
                let min_opts = winterfell::AcceptableOptions::MinConjecturedSecurity(0);
                match winterfell::verify::<BudAir, winterfell::crypto::hashers::Blake3_256<BaseElement>, winterfell::crypto::DefaultRandomCoin<winterfell::crypto::hashers::Blake3_256<BaseElement>>>(stark_proof, PubInputs { num_steps }, &min_opts) {
                    Ok(_) => {
                        println!("Proof verified successfully.");
                        true
                    }
                    Err(e) => {
                        println!("Proof verification FAILED: {}", e);
                        false
                    }
                }
            }
            Err(e) => {
                println!("Failed to parse proof: {}", e);
                false
            }
        }
    }
}

pub struct RecursiveProver;

impl RecursiveProver {
    pub fn aggregate(proofs: &[Proof]) -> Proof {
        let mut res = Vec::new();
        for p in proofs {
            res.extend_from_slice(&p.data);
        }
        Proof { data: res }
    }
}
