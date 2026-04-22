use winterfell::{
    Prover as WinterProver, Trace, TraceLayout,
    ConstraintCompositionCoefficients, 
    ProofOptions, AirContext, TransitionConstraintDegree,
    Assertion, EvaluationFrame, DefaultConstraintEvaluator,
    AuxTraceRandElements, TraceInfo, DefaultTraceLde,
    StarkDomain, TracePolyTable, matrix::ColMatrix
};
use winter_math::{fields::f128::BaseElement, FieldElement, ToElements, ExtensionOf};

#[derive(Clone, Debug)]
pub struct PubInputs {
    pub num_steps: usize,
}

impl ToElements<BaseElement> for PubInputs {
    fn to_elements(&self) -> Vec<BaseElement> {
        vec![BaseElement::from(self.num_steps as u64)]
    }
}

pub const TRACE_WIDTH: usize = 29; 

pub const COL_CLK: usize        = 0;
pub const COL_PC: usize         = 1;
pub const COL_OPCODE: usize     = 2;
pub const COL_RD_IDX: usize     = 3;
pub const COL_RS1_IDX: usize    = 4;
pub const COL_RS2_IDX: usize    = 5;
pub const COL_RS1_VAL: usize    = 6;
pub const COL_RS2_VAL: usize    = 7;
pub const COL_RD_VAL_NEW: usize = 8;
pub const COL_NEXT_PC: usize    = 9;
pub const COL_IMM: usize        = 10;

pub const COL_IS_ADD: usize     = 11;
pub const COL_IS_SUB: usize     = 12;
pub const COL_IS_MUL: usize     = 13;
pub const COL_IS_EQ: usize      = 14;
pub const COL_IS_LT: usize      = 15;
pub const COL_IS_JMP: usize     = 16;
pub const COL_IS_JNZ: usize     = 17;
pub const COL_IS_LOAD: usize    = 18;
pub const COL_IS_HALT: usize    = 19;
pub const COL_IS_ASSERT: usize  = 20;
pub const COL_IS_LOG: usize     = 21;
pub const COL_JNZ_COND: usize   = 22; 

pub const COL_REG_CLK: usize    = 23;
pub const COL_REG_IDX: usize    = 24;
pub const COL_REG_VAL: usize    = 25;
pub const COL_REG_IS_WRITE: usize = 26;
pub const COL_REG_ACTIVE: usize = 27;
pub const COL_REG_SAME: usize   = 28;

pub struct BudAir {
    context: AirContext<BaseElement>,
}

impl winterfell::Air for BudAir {
    type BaseField = BaseElement;
    type PublicInputs = PubInputs;

    fn new(trace_info: TraceInfo, _pub_inputs: PubInputs, options: ProofOptions) -> Self {
        let degrees = vec![TransitionConstraintDegree::new(1); 10];
        let aux_degrees = vec![TransitionConstraintDegree::new(1); 2];
        let context = AirContext::new_multi_segment(trace_info, degrees, aux_degrees, 2, 2, options);
        Self { context }
    }

    fn evaluate_transition<E: FieldElement<BaseField = Self::BaseField>>(
        &self,
        frame: &EvaluationFrame<E>,
        _periodic_values: &[E],
        result: &mut [E],
    ) {
        let cur = frame.current();
        let nxt = frame.next();
        let one = E::ONE;

        let clk = cur[COL_CLK];
        let pc = cur[COL_PC];
        let rs1_val = cur[COL_RS1_VAL];
        let rs2_val = cur[COL_RS2_VAL];
        let rd_val_new = cur[COL_RD_VAL_NEW];
        let imm = cur[COL_IMM];
        let next_pc = cur[COL_NEXT_PC];

        let is_cpu = cur[COL_IS_ADD] + cur[COL_IS_SUB] + cur[COL_IS_MUL] + cur[COL_IS_EQ] + 
                     cur[COL_IS_LT] + cur[COL_IS_JMP] + cur[COL_IS_JNZ] + cur[COL_IS_LOAD] + 
                     cur[COL_IS_HALT] + cur[COL_IS_ASSERT] + cur[COL_IS_LOG];

        result[0] = is_cpu * (nxt[COL_CLK] - clk - one);
        result[1] = is_cpu * (nxt[COL_PC] - next_pc);

        result[2] = cur[COL_IS_ADD] * (rd_val_new - (rs1_val + rs2_val));
        result[3] = cur[COL_IS_SUB] * (rd_val_new - (rs1_val - rs2_val));
        result[4] = cur[COL_IS_MUL] * (rd_val_new - (rs1_val * rs2_val));
        
        result[5] = cur[COL_IS_LOAD] * (rd_val_new - imm); 

        result[6] = cur[COL_IS_JMP] * (next_pc - (pc + imm));
        let jnz_cond = cur[COL_JNZ_COND]; 
        result[7] = cur[COL_IS_JNZ] * (next_pc - (jnz_cond * (pc + imm) + (one - jnz_cond) * (pc + one)));

        let r_val = cur[COL_REG_VAL];
        let r_active = cur[COL_REG_ACTIVE];
        let r_same = cur[COL_REG_SAME];
        let nr_val = nxt[COL_REG_VAL];
        let nr_active = nxt[COL_REG_ACTIVE];
        let nr_write = nxt[COL_REG_IS_WRITE];

        result[8] = r_active * nr_active * r_same * (one - nr_write) * (nr_val - r_val);
        
        let r_idx = cur[COL_REG_IDX];
        let nr_idx = nxt[COL_REG_IDX];
        result[9] = r_active * nr_active * r_same * (nr_idx - r_idx);
    }

    fn evaluate_aux_transition<F, E>(
        &self,
        main_frame: &EvaluationFrame<F>,
        aux_frame: &EvaluationFrame<E>,
        _periodic_values: &[F],
        rand_elements: &AuxTraceRandElements<E>,
        result: &mut [E],
    ) where
        F: FieldElement<BaseField = Self::BaseField>,
        E: FieldElement<BaseField = Self::BaseField> + ExtensionOf<F>,
    {
        let cur = main_frame.current();
        let acur = aux_frame.current();
        let anxt = aux_frame.next();

        let r = rand_elements.get_segment_elements(0);
        let alpha = r[0];
        let beta = r[1];
        let beta2 = beta.square();
        let beta3 = beta2 * beta;
        let beta4 = beta3 * beta;

        let get_term = |clk: F, idx: F, val: F, write: F| -> E {
            alpha + E::from(clk) * beta + E::from(idx) * beta2 + E::from(val) * beta3 + E::from(write) * beta4
        };

        let is_cpu_acc = cur[COL_IS_ADD] + cur[COL_IS_SUB] + cur[COL_IS_MUL] + cur[COL_IS_EQ] + 
                         cur[COL_IS_LT] + cur[COL_IS_JMP] + cur[COL_IS_JNZ] + cur[COL_IS_LOAD] + 
                         cur[COL_IS_ASSERT] + cur[COL_IS_LOG];
                         
        let term_s1 = get_term(cur[COL_CLK], cur[COL_RS1_IDX], cur[COL_RS1_VAL], F::ZERO);
        let term_s2 = get_term(cur[COL_CLK], cur[COL_RS2_IDX], cur[COL_RS2_VAL], F::ZERO);
        let term_rd = get_term(cur[COL_CLK], cur[COL_RD_IDX], cur[COL_RD_VAL_NEW], F::ONE);
        
        let cpu_packet = term_s1 * term_s2 * term_rd;
        result[0] = anxt[0] - acur[0] * (E::from(is_cpu_acc) * cpu_packet + (E::ONE - E::from(is_cpu_acc)));

        let is_reg = cur[COL_REG_ACTIVE];
        let reg_packet = get_term(cur[COL_REG_CLK], cur[COL_REG_IDX], cur[COL_REG_VAL], cur[COL_REG_IS_WRITE]);
        result[1] = anxt[1] - acur[1] * (E::from(is_reg) * reg_packet + (E::ONE - E::from(is_reg)));
    }

    fn get_assertions(&self) -> Vec<Assertion<Self::BaseField>> {
        vec![
            Assertion::single(COL_CLK, 0, BaseElement::ZERO),
            Assertion::single(COL_PC, 0, BaseElement::ZERO),
        ]
    }

    fn get_aux_assertions<E: FieldElement<BaseField = Self::BaseField>>(
        &self,
        _rand_elements: &AuxTraceRandElements<E>,
    ) -> Vec<Assertion<E>> {
        vec![
            Assertion::single(0, 0, E::ONE),
            Assertion::single(1, 0, E::ONE),
        ]
    }

    fn get_periodic_column_values(&self) -> Vec<Vec<Self::BaseField>> {
        vec![]
    }

    fn context(&self) -> &AirContext<Self::BaseField> {
        &self.context
    }
}

pub struct BudTrace {
    layout: TraceLayout,
    main_trace: ColMatrix<BaseElement>,
    meta: Vec<u8>,
}

impl Trace for BudTrace {
    type BaseField = BaseElement;

    fn layout(&self) -> &TraceLayout {
        &self.layout
    }

    fn main_segment(&self) -> &ColMatrix<Self::BaseField> {
        &self.main_trace
    }

    fn length(&self) -> usize {
        self.main_trace.num_rows()
    }

    fn meta(&self) -> &[u8] {
        &self.meta
    }

    fn build_aux_segment<E: FieldElement<BaseField = Self::BaseField> + ExtensionOf<Self::BaseField>>(
        &mut self,
        _aux_segments: &[ColMatrix<E>],
        rand_elements: &[E],
    ) -> Option<ColMatrix<E>> {
        let n = self.length();
        let mut acc_cpu = vec![E::ONE; n];
        let mut acc_reg = vec![E::ONE; n];
        let alpha = rand_elements[0];
        let beta = rand_elements[1];
        let beta2 = beta.square();
        let beta3 = beta2 * beta;
        let beta4 = beta3 * beta;
        
        let get_term = |clk: BaseElement, idx: BaseElement, val: BaseElement, write: BaseElement| -> E {
            alpha + E::from(clk) * beta + E::from(idx) * beta2 + E::from(val) * beta3 + E::from(write) * beta4
        };

        for i in 0..n-1 {
            let is_cpu = self.main_trace.get_column(COL_IS_ADD)[i] + self.main_trace.get_column(COL_IS_SUB)[i] + 
                         self.main_trace.get_column(COL_IS_MUL)[i] + self.main_trace.get_column(COL_IS_EQ)[i] + 
                         self.main_trace.get_column(COL_IS_LT)[i] + self.main_trace.get_column(COL_IS_JMP)[i] + 
                         self.main_trace.get_column(COL_IS_JNZ)[i] + self.main_trace.get_column(COL_IS_LOAD)[i] + 
                         self.main_trace.get_column(COL_IS_ASSERT)[i] + self.main_trace.get_column(COL_IS_LOG)[i];
            
            if is_cpu != BaseElement::ZERO {
                let t1 = get_term(self.main_trace.get_column(COL_CLK)[i], self.main_trace.get_column(COL_RS1_IDX)[i], self.main_trace.get_column(COL_RS1_VAL)[i], BaseElement::ZERO);
                let t2 = get_term(self.main_trace.get_column(COL_CLK)[i], self.main_trace.get_column(COL_RS2_IDX)[i], self.main_trace.get_column(COL_RS2_VAL)[i], BaseElement::ZERO);
                let t3 = get_term(self.main_trace.get_column(COL_CLK)[i], self.main_trace.get_column(COL_RD_IDX)[i], self.main_trace.get_column(COL_RD_VAL_NEW)[i], BaseElement::ONE);
                acc_cpu[i+1] = acc_cpu[i] * t1 * t2 * t3;
            } else {
                acc_cpu[i+1] = acc_cpu[i];
            }

            let is_reg = self.main_trace.get_column(COL_REG_ACTIVE)[i];
            if is_reg != BaseElement::ZERO {
                let t = get_term(self.main_trace.get_column(COL_REG_CLK)[i], self.main_trace.get_column(COL_REG_IDX)[i], self.main_trace.get_column(COL_REG_VAL)[i], self.main_trace.get_column(COL_REG_IS_WRITE)[i]);
                acc_reg[i+1] = acc_reg[i] * t;
            } else {
                acc_reg[i+1] = acc_reg[i];
            }
        }
        Some(ColMatrix::new(vec![acc_cpu, acc_reg]))
    }

    fn read_main_frame(&self, row_idx: usize, frame: &mut EvaluationFrame<Self::BaseField>) {
        let n = self.length();
        let next_idx = (row_idx + 1) % n;
        for i in 0..TRACE_WIDTH {
            frame.current_mut()[i] = self.main_trace.get_column(i)[row_idx];
            frame.next_mut()[i] = self.main_trace.get_column(i)[next_idx];
        }
    }
}

pub struct BudProver {
    options: ProofOptions,
}

impl WinterProver for BudProver {
    type BaseField = BaseElement;
    type Air = BudAir;
    type Trace = BudTrace;
    type HashFn = winterfell::crypto::hashers::Blake3_256<BaseElement>;
    type RandomCoin = winterfell::crypto::DefaultRandomCoin<Self::HashFn>;
    type TraceLde<E: FieldElement<BaseField = Self::BaseField>> = DefaultTraceLde<E, Self::HashFn>;
    type ConstraintEvaluator<'a, E: FieldElement<BaseField = Self::BaseField>> = DefaultConstraintEvaluator<'a, Self::Air, E>;

    fn get_pub_inputs(&self, trace: &Self::Trace) -> PubInputs {
        PubInputs { num_steps: trace.length() }
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

use crate::adapter::{Proof, ProverAdapter};

pub struct WinterfellAdapter;

impl ProverAdapter for WinterfellAdapter {
    fn prove(trace: &[bud_vm::Step], num_steps: usize) -> Proof {
        let bud_trace = Prover::generate_trace(trace);
        Prover::prove(bud_trace, num_steps)
    }

    fn verify(proof: &Proof, num_steps: usize) -> bool {
        Verifier::verify(proof, num_steps)
    }
}

pub struct Prover;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RegEvent {
    clk: u64,
    idx: u64,
    val: u64,
    is_write: bool,
    sub_clk: u8, 
}

impl Prover {
    pub fn generate_trace(trace: &[bud_vm::Step]) -> BudTrace {
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

        let mut cols = vec![vec![BaseElement::ZERO; num_rows]; TRACE_WIDTH];
        for (i, step) in trace.iter().enumerate() {
            let op = step.instruction.opcode as u8;
            cols[COL_CLK][i]       = BaseElement::from(i as u64);
            cols[COL_PC][i]        = BaseElement::from(step.pc as u64);
            cols[COL_OPCODE][i]    = BaseElement::from(op as u64);
            cols[COL_RD_IDX][i]    = BaseElement::from(step.dst_idx as u64);
            cols[COL_RS1_IDX][i]   = BaseElement::from(step.src1_idx as u64);
            cols[COL_RS2_IDX][i]   = BaseElement::from(step.src2_idx as u64);
            cols[COL_RS1_VAL][i]   = BaseElement::from(step.src1_val);
            cols[COL_RS2_VAL][i]   = BaseElement::from(step.src2_val);
            cols[COL_RD_VAL_NEW][i] = BaseElement::from(step.dst_val);
            cols[COL_NEXT_PC][i]   = BaseElement::from(step.next_pc as u64);
            
            cols[COL_IMM][i] = if step.instruction.imm < 0 {
                BaseElement::ZERO - BaseElement::from((-step.instruction.imm) as u64)
            } else {
                BaseElement::from(step.instruction.imm as u64)
            };

            match op {
                0x01 => cols[COL_IS_ADD][i] = BaseElement::ONE,
                0x02 => cols[COL_IS_SUB][i] = BaseElement::ONE,
                0x03 => cols[COL_IS_MUL][i] = BaseElement::ONE,
                0x0A => cols[COL_IS_EQ][i]  = BaseElement::ONE,
                0x0C => cols[COL_IS_LT][i]  = BaseElement::ONE,
                0x10 => cols[COL_IS_JMP][i] = BaseElement::ONE,
                0x11 => {
                    cols[COL_IS_JNZ][i] = BaseElement::ONE;
                    cols[COL_JNZ_COND][i] = if step.src1_val != 0 { BaseElement::ONE } else { BaseElement::ZERO };
                }
                0x14 => cols[COL_IS_LOAD][i] = BaseElement::ONE,
                0x00 => cols[COL_IS_HALT][i] = BaseElement::ONE,
                0x18 => cols[COL_IS_ASSERT][i] = BaseElement::ONE,
                0x1A => cols[COL_IS_LOG][i] = BaseElement::ONE,
                _ => {}
            }
        }
        for (i, e) in events.iter().enumerate() {
            cols[COL_REG_CLK][i]      = BaseElement::from(e.clk);
            cols[COL_REG_IDX][i]      = BaseElement::from(e.idx);
            cols[COL_REG_VAL][i]      = BaseElement::from(e.val);
            cols[COL_REG_IS_WRITE][i] = if e.is_write { BaseElement::ONE } else { BaseElement::ZERO };
            cols[COL_REG_ACTIVE][i]   = BaseElement::ONE;
            
            if i < num_rows - 1 {
                let next_idx = if i + 1 < n_reg { events[i+1].idx } else { 0 };
                if next_idx == e.idx {
                    cols[COL_REG_SAME][i] = BaseElement::ONE;
                } else {
                    cols[COL_REG_SAME][i] = BaseElement::ZERO;
                }
            }
        }
        for i in n_cpu..num_rows {
            cols[COL_CLK][i] = BaseElement::from(i as u64);
            if n_cpu > 0 {
                let last_pc = cols[COL_PC][n_cpu - 1];
                cols[COL_PC][i] = last_pc;
                cols[COL_NEXT_PC][i] = last_pc;
            }
            cols[COL_IS_HALT][i] = BaseElement::ONE;
        }
        let main_trace = ColMatrix::new(cols);
        let layout = TraceLayout::new(TRACE_WIDTH, [2], [2]);
        BudTrace { layout, main_trace, meta: vec![] }
    }

    pub fn prove(trace: BudTrace, _num_steps: usize) -> Proof {
        let options = ProofOptions::new(32, 32, 0, winterfell::FieldExtension::None, 4, 31);
        let prover = BudProver { options };
        let proof = prover.prove(trace).expect("Proving failed");
        Proof { data: proof.to_bytes() }
    }
}

pub struct Verifier;

impl Verifier {
    pub fn verify(proof: &Proof, num_steps: usize) -> bool {
        let parsed = winterfell::StarkProof::from_bytes(&proof.data);
        match parsed {
            Ok(stark_proof) => {
                let min_opts = winterfell::AcceptableOptions::MinConjecturedSecurity(0);
                winterfell::verify::<BudAir, winterfell::crypto::hashers::Blake3_256<BaseElement>, winterfell::crypto::DefaultRandomCoin<winterfell::crypto::hashers::Blake3_256<BaseElement>>>(stark_proof, PubInputs { num_steps }, &min_opts).is_ok()
            }
            Err(_) => false,
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
