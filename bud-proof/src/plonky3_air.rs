use p3_air::{Air, AirBuilder, BaseAir, WindowAccess};
use p3_field::PrimeCharacteristicRing;

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
    pub num_steps: usize,
}

impl<F> BaseAir<F> for BudAir {
    fn width(&self) -> usize {
        TRACE_WIDTH
    }
}

impl<AB: AirBuilder> Air<AB> for BudAir {
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let cur = main.current_slice();
        let nxt = main.next_slice();
        let one = AB::Expr::ONE;

        let clk = cur[COL_CLK].clone();
        let pc = cur[COL_PC].clone();
        let rs1_val = cur[COL_RS1_VAL].clone();
        let rs2_val = cur[COL_RS2_VAL].clone();
        let rd_val_new = cur[COL_RD_VAL_NEW].clone();
        let imm = cur[COL_IMM].clone();
        let next_pc = cur[COL_NEXT_PC].clone();

        let is_cpu = cur[COL_IS_ADD].clone() + cur[COL_IS_SUB].clone() + cur[COL_IS_MUL].clone() + 
                     cur[COL_IS_EQ].clone() + cur[COL_IS_LT].clone() + cur[COL_IS_JMP].clone() + 
                     cur[COL_IS_JNZ].clone() + cur[COL_IS_LOAD].clone() + cur[COL_IS_HALT].clone() + 
                     cur[COL_IS_ASSERT].clone() + cur[COL_IS_LOG].clone();

        builder.when_transition().assert_zero(is_cpu.clone() * (nxt[COL_CLK].clone() - clk - one.clone()));
        builder.when_transition().assert_zero(is_cpu.clone() * (nxt[COL_PC].clone() - next_pc.clone()));

        builder.when(cur[COL_IS_ADD].clone()).assert_eq(rd_val_new.clone(), rs1_val.clone() + rs2_val.clone());
        builder.when(cur[COL_IS_SUB].clone()).assert_eq(rd_val_new.clone(), rs1_val.clone() - rs2_val.clone());
        builder.when(cur[COL_IS_MUL].clone()).assert_eq(rd_val_new.clone(), rs1_val.clone() * rs2_val.clone());
        
        builder.when(cur[COL_IS_LOAD].clone()).assert_eq(rd_val_new.clone(), imm.clone());

        builder.when(cur[COL_IS_JMP].clone()).assert_eq(next_pc.clone(), pc.clone() + imm.clone());
        let jnz_cond = cur[COL_JNZ_COND].clone(); 
        builder.when(cur[COL_IS_JNZ].clone()).assert_eq(next_pc.clone(), jnz_cond.clone() * (pc.clone() + imm.clone()) + (one.clone() - jnz_cond) * (pc.clone() + one.clone()));

        let r_val = cur[COL_REG_VAL].clone();
        let r_active = cur[COL_REG_ACTIVE].clone();
        let r_same = cur[COL_REG_SAME].clone();
        let nr_val = nxt[COL_REG_VAL].clone();
        let nr_active = nxt[COL_REG_ACTIVE].clone();
        let nr_write = nxt[COL_REG_IS_WRITE].clone();

        builder.when_transition().assert_zero(r_active.clone() * nr_active.clone() * r_same.clone() * (one.clone() - nr_write) * (nr_val - r_val));
        
        let r_idx = cur[COL_REG_IDX].clone();
        let nr_idx = nxt[COL_REG_IDX].clone();
        builder.when_transition().assert_zero(r_active.clone() * nr_active.clone() * r_same.clone() * (nr_idx - r_idx));

        builder.when_first_row().assert_zero(cur[COL_CLK].clone());
        builder.when_first_row().assert_zero(cur[COL_PC].clone());
    }
}
