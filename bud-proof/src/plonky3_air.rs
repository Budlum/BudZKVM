use p3_air::{Air, AirBuilder, BaseAir, ExtensionBuilder, PermutationAirBuilder, WindowAccess};
use p3_field::PrimeCharacteristicRing;

pub const TRACE_WIDTH: usize = 49;

pub const COL_CLK: usize = 0;
pub const COL_PC: usize = 1;
pub const COL_OPCODE: usize = 2;
pub const COL_RD_IDX: usize = 3;
pub const COL_RS1_IDX: usize = 4;
pub const COL_RS2_IDX: usize = 5;
pub const COL_RS1_VAL: usize = 6;
pub const COL_RS2_VAL: usize = 7;
pub const COL_RD_VAL_NEW: usize = 8;
pub const COL_NEXT_PC: usize = 9;
pub const COL_IMM: usize = 10;

pub const COL_IS_ADD: usize = 11;
pub const COL_IS_SUB: usize = 12;
pub const COL_IS_MUL: usize = 13;
pub const COL_IS_EQ: usize = 14;
pub const COL_IS_LT: usize = 15;
pub const COL_IS_JMP: usize = 16;
pub const COL_IS_JNZ: usize = 17;
pub const COL_IS_LOAD: usize = 18;
pub const COL_IS_HALT: usize = 19;
pub const COL_IS_ASSERT: usize = 20;
pub const COL_IS_LOG: usize = 21;
pub const COL_JNZ_COND: usize = 22;

pub const COL_REG_CLK: usize = 23;
pub const COL_REG_IDX: usize = 24;
pub const COL_REG_VAL: usize = 25;
pub const COL_REG_IS_WRITE: usize = 26;
pub const COL_REG_ACTIVE: usize = 27;
pub const COL_REG_SAME: usize = 28;

pub const COL_IS_DIV: usize = 29;
pub const COL_IS_INV: usize = 30;
pub const COL_IS_AND: usize = 31;
pub const COL_IS_OR: usize = 32;
pub const COL_IS_XOR: usize = 33;
pub const COL_IS_NOT: usize = 34;
pub const COL_IS_NEQ: usize = 35;
pub const COL_IS_GT: usize = 36;
pub const COL_IS_LTE: usize = 37;
pub const COL_IS_GTE: usize = 38;
pub const COL_IS_STORE: usize = 39;
pub const COL_IS_PUSH: usize = 40;
pub const COL_IS_POP: usize = 41;
pub const COL_IS_CALL: usize = 42;
pub const COL_IS_RET: usize = 43;
pub const COL_IS_SREAD: usize = 44;
pub const COL_IS_SWRITE: usize = 45;
pub const COL_IS_POSEIDON: usize = 46;
pub const COL_IS_SYSCALL: usize = 47;
pub const COL_IS_VERIFY_MERKLE: usize = 48;

pub struct BudAir {
    pub num_steps: usize,
}

impl<F> BaseAir<F> for BudAir {
    fn width(&self) -> usize {
        TRACE_WIDTH
    }
}

impl<AB: PermutationAirBuilder> Air<AB> for BudAir {
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let cur = main.current_slice();
        let nxt = main.next_slice();
        let one: AB::Expr = AB::Expr::ONE;

        let clk: AB::Expr = cur[COL_CLK].into();
        let pc: AB::Expr = cur[COL_PC].into();
        let rs1_val: AB::Expr = cur[COL_RS1_VAL].into();
        let rs2_val: AB::Expr = cur[COL_RS2_VAL].into();
        let rd_val_new: AB::Expr = cur[COL_RD_VAL_NEW].into();
        let imm: AB::Expr = cur[COL_IMM].into();
        let next_pc: AB::Expr = cur[COL_NEXT_PC].into();

        let is_add: AB::Expr = cur[COL_IS_ADD].into();
        let is_sub: AB::Expr = cur[COL_IS_SUB].into();
        let is_mul: AB::Expr = cur[COL_IS_MUL].into();
        let is_div: AB::Expr = cur[COL_IS_DIV].into();
        let is_inv: AB::Expr = cur[COL_IS_INV].into();
        let is_and: AB::Expr = cur[COL_IS_AND].into();
        let is_or: AB::Expr = cur[COL_IS_OR].into();
        let is_xor: AB::Expr = cur[COL_IS_XOR].into();
        let is_not: AB::Expr = cur[COL_IS_NOT].into();
        let is_eq: AB::Expr = cur[COL_IS_EQ].into();
        let is_neq: AB::Expr = cur[COL_IS_NEQ].into();
        let is_lt: AB::Expr = cur[COL_IS_LT].into();
        let is_gt: AB::Expr = cur[COL_IS_GT].into();
        let is_lte: AB::Expr = cur[COL_IS_LTE].into();
        let is_gte: AB::Expr = cur[COL_IS_GTE].into();
        let is_jmp: AB::Expr = cur[COL_IS_JMP].into();
        let is_jnz: AB::Expr = cur[COL_IS_JNZ].into();
        let is_call: AB::Expr = cur[COL_IS_CALL].into();
        let is_ret: AB::Expr = cur[COL_IS_RET].into();
        let is_load: AB::Expr = cur[COL_IS_LOAD].into();
        let is_store: AB::Expr = cur[COL_IS_STORE].into();
        let is_push: AB::Expr = cur[COL_IS_PUSH].into();
        let is_pop: AB::Expr = cur[COL_IS_POP].into();
        let is_assert: AB::Expr = cur[COL_IS_ASSERT].into();
        let is_log: AB::Expr = cur[COL_IS_LOG].into();
        let is_sread: AB::Expr = cur[COL_IS_SREAD].into();
        let is_swrite: AB::Expr = cur[COL_IS_SWRITE].into();
        let is_poseidon: AB::Expr = cur[COL_IS_POSEIDON].into();
        let is_syscall: AB::Expr = cur[COL_IS_SYSCALL].into();
        let is_verify_merkle: AB::Expr = cur[COL_IS_VERIFY_MERKLE].into();
        let is_halt: AB::Expr = cur[COL_IS_HALT].into();
        let nxt_is_halt: AB::Expr = nxt[COL_IS_HALT].into();
        let nxt_clk: AB::Expr = nxt[COL_CLK].into();
        let nxt_pc: AB::Expr = nxt[COL_PC].into();

        let is_cpu = is_add.clone()
            + is_sub.clone()
            + is_mul.clone()
            + is_div.clone()
            + is_inv.clone()
            + is_and.clone()
            + is_or.clone()
            + is_xor.clone()
            + is_not.clone()
            + is_eq.clone()
            + is_neq.clone()
            + is_lt.clone()
            + is_gt.clone()
            + is_lte.clone()
            + is_gte.clone()
            + is_jmp.clone()
            + is_jnz.clone()
            + is_call.clone()
            + is_ret.clone()
            + is_load.clone()
            + is_store.clone()
            + is_push.clone()
            + is_pop.clone()
            + is_assert.clone()
            + is_log.clone()
            + is_sread.clone()
            + is_swrite.clone()
            + is_poseidon.clone()
            + is_syscall.clone()
            + is_verify_merkle.clone()
            + is_halt.clone();

        builder
            .when_transition()
            .assert_zero(is_cpu.clone() * (nxt_clk.clone() - clk.clone() - one.clone()));
        builder
            .when_transition()
            .assert_zero(is_cpu.clone() * (nxt_pc.clone() - next_pc.clone()));

        builder
            .when(is_add)
            .assert_eq(rd_val_new.clone(), rs1_val.clone() + rs2_val.clone());
        builder
            .when(is_sub)
            .assert_eq(rd_val_new.clone(), rs1_val.clone() - rs2_val.clone());
        builder
            .when(is_mul)
            .assert_eq(rd_val_new.clone(), rs1_val.clone() * rs2_val.clone());

        builder
            .when(is_inv)
            .assert_eq(rd_val_new.clone() * rs1_val.clone(), one.clone());
        builder
            .when(is_eq)
            .assert_zero(rd_val_new.clone() * (rs1_val.clone() - rs2_val.clone()));
        builder
            .when(is_neq)
            .assert_zero((one.clone() - rd_val_new.clone()) * (rs1_val.clone() - rs2_val.clone()));

        builder
            .when(is_load)
            .assert_eq(rd_val_new.clone(), imm.clone());

        builder
            .when(is_jmp)
            .assert_eq(next_pc.clone(), pc.clone() + imm.clone());
        let jnz_cond: AB::Expr = cur[COL_JNZ_COND].into();
        builder.when(is_jnz).assert_eq(
            next_pc.clone(),
            jnz_cond.clone() * (pc.clone() + imm.clone())
                + (one.clone() - jnz_cond.clone()) * (pc.clone() + one.clone()),
        );

        builder
            .when_transition()
            .when(is_halt.clone())
            .assert_eq(nxt_is_halt, one.clone());
        builder
            .when_transition()
            .when(is_halt.clone())
            .assert_eq(nxt_pc, cur[COL_PC].into());

        let r_val: AB::Expr = cur[COL_REG_VAL].into();
        let r_active: AB::Expr = cur[COL_REG_ACTIVE].into();
        let r_same: AB::Expr = cur[COL_REG_SAME].into();
        let nr_val: AB::Expr = nxt[COL_REG_VAL].into();
        let nr_active: AB::Expr = nxt[COL_REG_ACTIVE].into();
        let nr_write: AB::Expr = nxt[COL_REG_IS_WRITE].into();
        let r_idx: AB::Expr = cur[COL_REG_IDX].into();
        let nr_idx: AB::Expr = nxt[COL_REG_IDX].into();

        builder.when_transition().assert_zero(
            r_active.clone()
                * nr_active.clone()
                * r_same.clone()
                * (one.clone() - nr_write)
                * (nr_val - r_val),
        );
        builder
            .when_transition()
            .assert_zero(r_active.clone() * nr_active.clone() * r_same.clone() * (nr_idx - r_idx));

        let cur_clk: AB::Expr = cur[COL_CLK].into();
        let cur_pc: AB::Expr = cur[COL_PC].into();
        builder.when_first_row().assert_zero(cur_clk);
        builder.when_first_row().assert_zero(cur_pc);

        let perm = builder.permutation();
        let perm_cur = perm.current_slice();
        let perm_nxt = perm.next_slice();
        let rand = builder.permutation_randomness();
        if rand.len() >= 2 && perm_cur.len() >= 2 && perm_nxt.len() >= 2 {
            let alpha = rand[0];
            let beta = rand[1];

            let rs1_idx: AB::Expr = cur[COL_RS1_IDX].into();
            let rs2_idx: AB::Expr = cur[COL_RS2_IDX].into();
            let rd_idx: AB::Expr = cur[COL_RD_IDX].into();
            let reg_clk: AB::Expr = cur[COL_REG_CLK].into();
            let reg_idx: AB::Expr = cur[COL_REG_IDX].into();
            let reg_val: AB::Expr = cur[COL_REG_VAL].into();
            let reg_is_write: AB::Expr = cur[COL_REG_IS_WRITE].into();

            let alpha_expr: AB::ExprEF = alpha.into();
            let beta_expr: AB::ExprEF = beta.into();
            let b2 = beta_expr.clone() * beta_expr.clone();
            let b3 = b2.clone() * beta_expr.clone();
            let b4 = b3.clone() * beta_expr.clone();

            let term =
                |clk: AB::Expr, idx: AB::Expr, val: AB::Expr, is_write: AB::Expr| -> AB::ExprEF {
                    let clk: AB::ExprEF = clk.into();
                    let idx: AB::ExprEF = idx.into();
                    let val: AB::ExprEF = val.into();
                    let is_write: AB::ExprEF = is_write.into();
                    alpha_expr.clone()
                        + beta_expr.clone() * clk
                        + b2.clone() * idx
                        + b3.clone() * val
                        + b4.clone() * is_write
                };

            let cpu_packet = term(clk.clone(), rs1_idx, rs1_val.clone(), AB::Expr::ZERO)
                * term(clk.clone(), rs2_idx, rs2_val.clone(), AB::Expr::ZERO)
                * term(clk.clone(), rd_idx, rd_val_new.clone(), one.clone());
            let reg_packet = term(reg_clk, reg_idx, reg_val, reg_is_write);

            let cpu_acc_cur: AB::ExprEF = perm_cur[0].into();
            let cpu_acc_nxt: AB::ExprEF = perm_nxt[0].into();
            let reg_acc_cur: AB::ExprEF = perm_cur[1].into();
            let reg_acc_nxt: AB::ExprEF = perm_nxt[1].into();
            let is_cpu_ext: AB::ExprEF = is_cpu.clone().into();
            let not_cpu_ext: AB::ExprEF = (one.clone() - is_cpu).into();
            let r_active_ext: AB::ExprEF = r_active.clone().into();
            let not_r_active_ext: AB::ExprEF = (one - r_active).into();

            builder.when_first_row().assert_one_ext(cpu_acc_cur.clone());
            builder.when_first_row().assert_one_ext(reg_acc_cur.clone());
            builder.when_transition().assert_zero_ext(
                cpu_acc_nxt - cpu_acc_cur * (is_cpu_ext * cpu_packet + not_cpu_ext),
            );
            builder.when_transition().assert_zero_ext(
                reg_acc_nxt - reg_acc_cur * (r_active_ext * reg_packet + not_r_active_ext),
            );
        }
    }
}
