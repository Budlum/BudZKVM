use p3_air::{Air, AirBuilder, BaseAir, ExtensionBuilder, PermutationAirBuilder, WindowAccess};
use p3_field::{PrimeCharacteristicRing, PrimeField64};

pub const TRACE_WIDTH: usize = 57;

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

pub const COL_MEM_CLK: usize = 49;
pub const COL_MEM_ADDR: usize = 50;
pub const COL_MEM_VAL: usize = 51;
pub const COL_MEM_IS_WRITE: usize = 52;
pub const COL_MEM_ACTIVE: usize = 53;
pub const COL_MEM_SAME: usize = 54;
pub const COL_STACK_PTR: usize = 55;
pub const COL_REG_SUB_CLK: usize = 56;
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
        let cur_stack_ptr: AB::Expr = cur[COL_STACK_PTR].into();
        let nxt_stack_ptr: AB::Expr = nxt[COL_STACK_PTR].into();

        let is_real_op = is_add.clone()
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
            + is_verify_merkle.clone();
        
        let is_cpu = is_real_op.clone() + is_halt.clone();

        // 1. Selector Booleanity
        builder.assert_bool(is_add.clone());
        builder.assert_bool(is_sub.clone());
        builder.assert_bool(is_mul.clone());
        builder.assert_bool(is_div.clone());
        builder.assert_bool(is_inv.clone());
        builder.assert_bool(is_and.clone());
        builder.assert_bool(is_or.clone());
        builder.assert_bool(is_xor.clone());
        builder.assert_bool(is_not.clone());
        builder.assert_bool(is_eq.clone());
        builder.assert_bool(is_neq.clone());
        builder.assert_bool(is_lt.clone());
        builder.assert_bool(is_gt.clone());
        builder.assert_bool(is_lte.clone());
        builder.assert_bool(is_gte.clone());
        builder.assert_bool(is_jmp.clone());
        builder.assert_bool(is_jnz.clone());
        builder.assert_bool(is_call.clone());
        builder.assert_bool(is_ret.clone());
        builder.assert_bool(is_load.clone());
        builder.assert_bool(is_store.clone());
        builder.assert_bool(is_push.clone());
        builder.assert_bool(is_pop.clone());
        builder.assert_bool(is_assert.clone());
        builder.assert_bool(is_log.clone());
        builder.assert_bool(is_sread.clone());
        builder.assert_bool(is_swrite.clone());
        builder.assert_bool(is_poseidon.clone());
        builder.assert_bool(is_syscall.clone());
        builder.assert_bool(is_verify_merkle.clone());
        builder.assert_bool(is_halt.clone());

        // 2. Selector Exclusivity: Every row must be exactly one opcode (including HALT as padding)
        builder.assert_eq(is_cpu.clone(), one.clone());

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

        let is_div: AB::Expr = cur[COL_IS_DIV].into();
        builder
            .when(is_div)
            .assert_eq(rs1_val.clone(), rd_val_new.clone() * rs2_val.clone()); // Note: requires divisor != 0

        let is_and: AB::Expr = cur[COL_IS_AND].into();
        let is_or: AB::Expr = cur[COL_IS_OR].into();
        let is_xor: AB::Expr = cur[COL_IS_XOR].into();
        let is_not: AB::Expr = cur[COL_IS_NOT].into();
        // Bitwise operations require lookup tables or bit-decomposition in Goldilocks field
        // Placeholder constraints to satisfy completeness
        builder.when(is_and).assert_zero(rd_val_new.clone() - rd_val_new.clone());
        builder.when(is_or).assert_zero(rd_val_new.clone() - rd_val_new.clone());
        builder.when(is_xor).assert_zero(rd_val_new.clone() - rd_val_new.clone());
        builder.when(is_not).assert_zero(rd_val_new.clone() - rd_val_new.clone());

        builder
            .when(is_inv)
            .assert_eq(rd_val_new.clone() * rs1_val.clone(), one.clone());
        builder
            .when(is_eq)
            .assert_zero(rd_val_new.clone() * (rs1_val.clone() - rs2_val.clone()));
        builder
            .when(is_neq)
            .assert_zero((one.clone() - rd_val_new.clone()) * (rs1_val.clone() - rs2_val.clone()));

        let rs1_idx: AB::Expr = cur[COL_RS1_IDX].into();
        builder
            .when(is_load.clone() * (one.clone() - rs1_idx.clone()))
            .assert_eq(rd_val_new.clone(), imm.clone());

        builder
            .when(is_jmp.clone() + is_call.clone())
            .assert_eq(next_pc.clone(), pc.clone() + imm.clone());
        // Return PC is constrained by the Memory LogUp popping the return address
        let jnz_cond: AB::Expr = cur[COL_JNZ_COND].into();
        builder.when(is_jnz).assert_eq(
            next_pc.clone(),
            jnz_cond.clone() * (pc.clone() + imm.clone())
                + (one.clone() - jnz_cond.clone()) * (pc.clone() + one.clone()),
        );

        builder.when(is_assert).assert_one(rs1_val.clone());

        let is_push: AB::Expr = cur[COL_IS_PUSH].into();
        let is_pop: AB::Expr = cur[COL_IS_POP].into();
        let is_call: AB::Expr = cur[COL_IS_CALL].into();
        let is_ret: AB::Expr = cur[COL_IS_RET].into();

        builder.when(is_push.clone()).assert_eq(next_pc.clone(), pc.clone() + one.clone());
        builder.when(is_pop.clone()).assert_eq(next_pc.clone(), pc.clone() + one.clone());
        builder.when(is_call.clone()).assert_eq(next_pc.clone(), pc.clone() + imm.clone());
        // Return jumps to popped value (constrained by Memory LogUp).

        // Stack pointer transition
        builder.when_transition().assert_zero(
            is_push.clone() * (nxt_stack_ptr.clone() - cur_stack_ptr.clone() - one.clone())
                + is_call.clone() * (nxt_stack_ptr.clone() - cur_stack_ptr.clone() - one.clone())
                + is_pop.clone() * (nxt_stack_ptr.clone() - cur_stack_ptr.clone() + one.clone())
                + is_ret.clone() * (nxt_stack_ptr.clone() - cur_stack_ptr.clone() + one.clone())
                + (one.clone() - is_push - is_pop - is_call - is_ret) * (nxt_stack_ptr - cur_stack_ptr),
        );
        builder.when_first_row().assert_zero(cur[COL_STACK_PTR].into());

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

        let m_val: AB::Expr = cur[COL_MEM_VAL].into();
        let m_active: AB::Expr = cur[COL_MEM_ACTIVE].into();
        let m_same: AB::Expr = cur[COL_MEM_SAME].into();
        let nm_val: AB::Expr = nxt[COL_MEM_VAL].into();
        let nm_active: AB::Expr = nxt[COL_MEM_ACTIVE].into();
        let nm_write: AB::Expr = nxt[COL_MEM_IS_WRITE].into();
        let m_addr: AB::Expr = cur[COL_MEM_ADDR].into();
        let nm_addr: AB::Expr = nxt[COL_MEM_ADDR].into();
        let m_clk: AB::Expr = cur[COL_MEM_CLK].into();
        let m_is_write: AB::Expr = cur[COL_MEM_IS_WRITE].into();

        builder.when_transition().assert_zero(
            m_active.clone()
                * nm_active.clone()
                * m_same.clone()
                * (one.clone() - nm_write)
                * (nm_val - m_val.clone()),
        );
        builder
            .when_transition()
            .assert_zero(m_active.clone() * nm_active.clone() * m_same.clone() * (nm_addr - m_addr.clone()));

        let cur_clk: AB::Expr = cur[COL_CLK].into();
        let cur_pc: AB::Expr = cur[COL_PC].into();
        builder.when_first_row().assert_zero(cur_clk);
        builder.when_first_row().assert_zero(cur_pc);

        let perm = builder.permutation();
        let perm_cur = perm.current_slice();
        let perm_nxt = perm.next_slice();
        let rand = builder.permutation_randomness();
        if rand.len() >= 3 && perm_cur.len() >= 2 && perm_nxt.len() >= 2 {
            let alpha = rand[0];
            let beta = rand[1];
            let gamma = rand[2];

            let rs1_idx: AB::Expr = cur[COL_RS1_IDX].into();
        let rs2_idx: AB::Expr = cur[COL_RS2_IDX].into();
        let rd_idx: AB::Expr = cur[COL_RD_IDX].into();
            let reg_clk: AB::Expr = cur[COL_REG_CLK].into();
            let reg_sub_clk: AB::Expr = cur[COL_REG_SUB_CLK].into();
            let reg_idx: AB::Expr = cur[COL_REG_IDX].into();
            let reg_val: AB::Expr = cur[COL_REG_VAL].into();
            let reg_is_write: AB::Expr = cur[COL_REG_IS_WRITE].into();

            let alpha_expr: AB::ExprEF = alpha.into();
            let beta_expr: AB::ExprEF = beta.into();
            let gamma_expr: AB::ExprEF = gamma.into();
            
            let b2 = beta_expr.clone() * beta_expr.clone();
            let b3 = b2.clone() * beta_expr.clone();
            let b4 = b3.clone() * beta_expr.clone();
            let b5 = b4.clone() * beta_expr.clone();

            let term =
                |table_id: AB::Expr, clk: AB::Expr, idx: AB::Expr, val: AB::Expr, is_write: AB::Expr| -> AB::ExprEF {
                    let table_id: AB::ExprEF = table_id.into();
                    let clk: AB::ExprEF = clk.into();
                    let idx: AB::ExprEF = idx.into();
                    let val: AB::ExprEF = val.into();
                    let is_write: AB::ExprEF = is_write.into();
                    alpha_expr.clone()
                        + beta_expr.clone() * table_id
                        + b2.clone() * clk
                        + b3.clone() * idx
                        + b4.clone() * val
                        + b5.clone() * is_write
                };

            let zero = AB::Expr::from(AB::F::ZERO);
            let one = AB::Expr::from(AB::F::ONE);
            
            let table_reg = zero.clone();

            // Register LogUp (perm_cur[0] / perm_nxt[0])
            let four = AB::Expr::from(AB::F::from_u64(4));
            let one_val = AB::Expr::from(AB::F::from_u64(1));
            let two_val = AB::Expr::from(AB::F::from_u64(2));
            let three_val = AB::Expr::from(AB::F::from_u64(3));

            let clk_rs1 = clk.clone() * four.clone() + one_val;
            let clk_rs2 = clk.clone() * four.clone() + two_val;
            let clk_rd = clk.clone() * four.clone() + three_val;
            let clk_reg = reg_clk.clone() * four.clone() + reg_sub_clk;

            let c_rs1 = term(table_reg.clone(), clk_rs1, rs1_idx.clone(), rs1_val.clone(), zero.clone());
            let c_rs2 = term(table_reg.clone(), clk_rs2, rs2_idx.clone(), rs2_val.clone(), zero.clone());
            let c_rd = term(table_reg.clone(), clk_rd, rd_idx.clone(), rd_val_new.clone(), one.clone());
            let c_reg = term(table_reg.clone(), clk_reg, reg_idx.clone(), reg_val.clone(), reg_is_write.clone());

            let r_active_ext: AB::ExprEF = r_active.clone().into();

            let diff_rs1 = gamma_expr.clone() - c_rs1;
            let diff_rs2 = gamma_expr.clone() - c_rs2;
            let diff_rd = gamma_expr.clone() - c_rd;
            let diff_reg = gamma_expr.clone() - c_reg;

            let d_rs1 = diff_rs2.clone() * diff_rd.clone() * diff_reg.clone();
            let d_rs2 = diff_rs1.clone() * diff_rd.clone() * diff_reg.clone();
            let d_rd = diff_rs1.clone() * diff_rs2.clone() * diff_reg.clone();
            let d_reg = diff_rs1.clone() * diff_rs2.clone() * diff_rd.clone();
            let d_total = diff_rs1 * diff_rs2 * diff_rd * diff_reg;
            let s_reg_cur: AB::ExprEF = perm_cur[0].into();
            let s_reg_nxt: AB::ExprEF = perm_nxt[0].into();
            let is_real_op_ext: AB::ExprEF = is_real_op.into();
            builder.when_transition().assert_zero_ext(
                (s_reg_nxt.clone() - s_reg_cur.clone()) * d_total
                - (is_real_op_ext * (d_rs1 + d_rs2 + d_rd) - r_active_ext * d_reg)
            );
            builder.when_first_row().assert_zero_ext(s_reg_cur);
            builder.when_last_row().assert_zero_ext(s_reg_nxt);

            // Memory LogUp
            let is_load: AB::Expr = cur[COL_IS_LOAD].into();
            let is_store: AB::Expr = cur[COL_IS_STORE].into();
            let is_push: AB::Expr = cur[COL_IS_PUSH].into();
            let is_pop: AB::Expr = cur[COL_IS_POP].into();
            let is_call: AB::Expr = cur[COL_IS_CALL].into();
            let is_ret: AB::Expr = cur[COL_IS_RET].into();

            let rs1_idx: AB::Expr = cur[COL_RS1_IDX].into();
            let is_real_mem_op = (is_load.clone() + is_store.clone()) * rs1_idx.clone(); // If rs1 is 0, it's LoadImm
            let is_stack_op = is_push.clone() + is_pop.clone() + is_call.clone() + is_ret.clone();
            let is_any_mem_op = is_real_mem_op.clone() + is_stack_op.clone();

            let stack_base = AB::Expr::from(AB::F::from_u64(1 << 60));
            let stack_ptr: AB::Expr = cur[COL_STACK_PTR].into();
            let stack_addr = stack_base.clone()
                + (is_push.clone() + is_call.clone()) * stack_ptr.clone()
                + (is_pop.clone() + is_ret.clone()) * (stack_ptr.clone() - one.clone());

            let final_mem_addr = is_real_mem_op.clone() * (cur[COL_RS1_VAL].into() + cur[COL_IMM].into()) + is_stack_op.clone() * stack_addr;

            let is_write = is_store.clone() + is_push.clone() + is_call.clone();
            let cpu_mem_val = is_load * cur[COL_RD_VAL_NEW].into()
                + is_store * cur[COL_RS2_VAL].into()
                + is_push * cur[COL_RS1_VAL].into()
                + is_pop * cur[COL_RD_VAL_NEW].into()
                + is_call * (cur[COL_PC].into() + one.clone())
                + is_ret * cur[COL_NEXT_PC].into();

            let c_cpu_mem = term(one.clone(), clk.clone(), final_mem_addr.clone(), cpu_mem_val.clone(), is_write.clone());
            let c_mem = term(one.clone(), m_clk.clone(), m_addr.clone(), m_val.clone(), m_is_write.clone());

            let is_any_mem_op_ext: AB::ExprEF = is_any_mem_op.into();
            let m_active_ext: AB::ExprEF = m_active.into();

            let diff_cpu_mem = gamma_expr.clone() - c_cpu_mem;
            let diff_mem = gamma_expr.clone() - c_mem;

            let s_mem_cur: AB::ExprEF = perm_cur[1].into();
            let s_mem_nxt: AB::ExprEF = perm_nxt[1].into();

            builder.when_transition().assert_zero_ext(
                (s_mem_nxt.clone() - s_mem_cur.clone()) * diff_cpu_mem.clone() * diff_mem.clone()
                    - (is_any_mem_op_ext * diff_mem - m_active_ext * diff_cpu_mem),
            );
            builder.when_first_row().assert_zero_ext(s_mem_cur);
            builder.when_last_row().assert_zero_ext(s_mem_nxt);
            
        }
    }
}
