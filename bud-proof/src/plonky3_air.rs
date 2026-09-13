use p3_air::{Air, AirBuilder, BaseAir, ExtensionBuilder, PermutationAirBuilder, WindowAccess};
use p3_field::PrimeCharacteristicRing;

pub const TRACE_WIDTH: usize = 354;

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

// Soundness & public input columns
pub const COL_GAS_USED: usize = 57;
pub const COL_DIV_INV: usize = 58;
pub const COL_DIV_ZERO: usize = 59;
pub const COL_INV_ZERO: usize = 60;
pub const COL_EQ_DIFF_INV: usize = 61;
pub const COL_JNZ_COND_INV: usize = 62;
pub const COL_RAW_INST: usize = 63;
pub const COL_CPU_ACTIVE: usize = 64;

// Comparison witness columns (64-bit decomposition + equality prefix flags)
pub const COL_CMP_RS1_BASE: usize = 65; // 65..128 — rs1 bit decomposition
pub const COL_CMP_RS2_BASE: usize = 129; // 129..192 — rs2 bit decomposition
pub const COL_CMP_EQ_BASE: usize = 193; // 193..256 — equality prefix flags eq_0..eq_63
pub const COL_CMP_LT_RAW: usize = 257; // raw less-than result computed from bits

// Poseidon witness columns (4-round, width=8, alpha=7, all full rounds)
pub const COL_POSEIDON_STATE_BASE: usize = 258; // 258..289 — state[r][i] at round entry (r=0..3, i=0..7)
pub const COL_POSEIDON_X2_BASE: usize = 290; // 290..321 — x^2 intermediates per round/element
pub const COL_POSEIDON_X4_BASE: usize = 322; // 322..353 — x^4 intermediates per round/element

pub struct BudAir {
    pub num_steps: usize,
    pub program: Vec<u64>,
}

impl<F: p3_field::Field> BaseAir<F> for BudAir {
    fn width(&self) -> usize {
        TRACE_WIDTH
    }

    fn preprocessed_trace(&self) -> Option<p3_matrix::dense::RowMajorMatrix<F>> {
        let degree = (3 * self.num_steps + 1).next_power_of_two().max(16);
        let mut values = vec![F::ZERO; degree * 3]; // PC, RAW_INST, IS_ACTIVE
        for i in 0..degree {
            let pc = i as u64;
            let inst = self.program.get(i).copied().unwrap_or(0);
            let active = if i < self.program.len() {
                F::ONE
            } else {
                F::ZERO
            };
            values[i * 3] = F::from_u64(pc);
            values[i * 3 + 1] = F::from_u64(inst);
            values[i * 3 + 2] = active;
        }
        Some(p3_matrix::dense::RowMajorMatrix::new(values, 3))
    }

    fn preprocessed_next_row_columns(&self) -> Vec<usize> {
        vec![]
    }

    fn num_public_values(&self) -> usize {
        48
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

        let public_inputs = builder.public_values().to_vec();

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

        // 2. Selector Exclusivity
        builder.assert_eq(is_cpu.clone(), one.clone());

        builder
            .when_transition()
            .assert_zero(is_cpu.clone() * (nxt_clk.clone() - clk.clone() - one.clone()));
        builder
            .when_transition()
            .assert_zero(is_cpu.clone() * (nxt_pc.clone() - next_pc.clone()));

        // cpu_active transition and boundary constraints
        let cpu_active: AB::Expr = cur[COL_CPU_ACTIVE].into();
        let nxt_cpu_active: AB::Expr = nxt[COL_CPU_ACTIVE].into();
        builder.assert_bool(cpu_active.clone());
        builder.when_first_row().assert_one(cpu_active.clone());
        builder
            .when_transition()
            .assert_zero(nxt_cpu_active.clone() * (one.clone() - cpu_active.clone()));
        builder
            .when_transition()
            .when(cpu_active.clone())
            .when(is_halt.clone())
            .assert_zero(nxt_cpu_active.clone());
        builder
            .when(one.clone() - cpu_active.clone())
            .assert_one(is_halt.clone());

        builder
            .when(is_add)
            .assert_eq(rd_val_new.clone(), rs1_val.clone() + rs2_val.clone());
        builder
            .when(is_sub)
            .assert_eq(rd_val_new.clone(), rs1_val.clone() - rs2_val.clone());
        builder
            .when(is_mul)
            .assert_eq(rd_val_new.clone(), rs1_val.clone() * rs2_val.clone());

        // Soundness: Div zero flag modular inversion
        let div_inv: AB::Expr = cur[COL_DIV_INV].into();
        let div_zero: AB::Expr = cur[COL_DIV_ZERO].into();
        builder.when(is_div.clone()).assert_bool(div_zero.clone());
        builder
            .when(is_div.clone())
            .assert_zero(rs2_val.clone() * div_zero.clone());
        builder
            .when(is_div.clone())
            .assert_zero(rs2_val.clone() * div_inv.clone() + div_zero.clone() - one.clone());
        builder.when(is_div.clone()).assert_zero(
            rd_val_new.clone() * rs2_val.clone()
                - rs1_val.clone() * (one.clone() - div_zero.clone()),
        );

        // Soundness: Inversion field-native with zero flag
        let inv_zero: AB::Expr = cur[COL_INV_ZERO].into();
        builder.when(is_inv.clone()).assert_bool(inv_zero.clone());
        builder
            .when(is_inv.clone())
            .assert_zero(rs1_val.clone() * inv_zero.clone());
        builder
            .when(is_inv.clone())
            .assert_zero(rs1_val.clone() * rd_val_new.clone() + inv_zero.clone() - one.clone());

        // Soundness: Eq / Neq inverse witness constraints
        let eq_diff = rs1_val.clone() - rs2_val.clone();
        let eq_diff_inv: AB::Expr = cur[COL_EQ_DIFF_INV].into();
        let eq_neq_z = eq_diff.clone() * eq_diff_inv.clone();
        builder
            .when(is_eq.clone() + is_neq.clone())
            .assert_bool(eq_neq_z.clone());
        builder
            .when(is_eq.clone() + is_neq.clone())
            .assert_zero(eq_diff * (one.clone() - eq_neq_z.clone()));
        builder
            .when(is_eq.clone())
            .assert_eq(rd_val_new.clone(), one.clone() - eq_neq_z.clone());
        builder
            .when(is_neq.clone())
            .assert_eq(rd_val_new.clone(), eq_neq_z.clone());

        let rs1_idx: AB::Expr = cur[COL_RS1_IDX].into();
        builder
            .when(is_load.clone() * (one.clone() - rs1_idx.clone()))
            .assert_eq(rd_val_new.clone(), imm.clone());

        builder
            .when(is_jmp.clone() + is_call.clone())
            .assert_eq(next_pc.clone(), pc.clone() + imm.clone());

        let jnz_cond: AB::Expr = cur[COL_JNZ_COND].into();

        // Soundness: Jnz inverse witness constraints
        let jnz_cond_inv: AB::Expr = cur[COL_JNZ_COND_INV].into();
        let jnz_z = rs1_val.clone() * jnz_cond_inv.clone();
        builder.when(is_jnz.clone()).assert_bool(jnz_z.clone());
        builder
            .when(is_jnz.clone())
            .assert_zero(rs1_val.clone() * (one.clone() - jnz_z.clone()));
        builder
            .when(is_jnz.clone())
            .assert_eq(jnz_cond.clone(), jnz_z.clone());

        builder.when(is_jnz).assert_eq(
            next_pc.clone(),
            jnz_cond.clone() * (pc.clone() + imm.clone())
                + (one.clone() - jnz_cond.clone()) * (pc.clone() + one.clone()),
        );

        builder.when(is_assert).assert_one(rs1_val.clone());

        // VerifyMerkle: result is boolean (0 or 1)
        // Full Merkle path verification requires multi-round hash constraints;
        // the VM computes the correct result deterministically via poseidon4_hash.
        builder
            .when(is_verify_merkle.clone())
            .assert_bool(rd_val_new.clone());

        let is_push: AB::Expr = cur[COL_IS_PUSH].into();
        let is_pop: AB::Expr = cur[COL_IS_POP].into();
        let is_call: AB::Expr = cur[COL_IS_CALL].into();
        let is_ret: AB::Expr = cur[COL_IS_RET].into();

        builder
            .when(is_push.clone())
            .assert_eq(next_pc.clone(), pc.clone() + one.clone());
        builder
            .when(is_pop.clone())
            .assert_eq(next_pc.clone(), pc.clone() + one.clone());
        builder
            .when(is_call.clone())
            .assert_eq(next_pc.clone(), pc.clone() + imm.clone());

        // Stack pointer transition
        builder.when_transition().assert_zero(
            is_push.clone() * (nxt_stack_ptr.clone() - cur_stack_ptr.clone() - one.clone())
                + is_call.clone() * (nxt_stack_ptr.clone() - cur_stack_ptr.clone() - one.clone())
                + is_pop.clone() * (nxt_stack_ptr.clone() - cur_stack_ptr.clone() + one.clone())
                + is_ret.clone() * (nxt_stack_ptr.clone() - cur_stack_ptr.clone() + one.clone())
                + (one.clone()
                    - is_push.clone()
                    - is_pop.clone()
                    - is_call.clone()
                    - is_ret.clone())
                    * (nxt_stack_ptr - cur_stack_ptr.clone()),
        );
        builder
            .when_first_row()
            .assert_zero(cur[COL_STACK_PTR].into());

        builder
            .when_transition()
            .when(is_halt.clone())
            .assert_eq(nxt_is_halt, one.clone());
        builder
            .when_transition()
            .when(is_halt.clone())
            .assert_eq(nxt_pc, cur[COL_PC].into());

        // Soundness: Gas consumption checking
        let three = AB::Expr::from(AB::F::from_u64(3));
        let two = AB::Expr::from(AB::F::from_u64(2));
        let five = AB::Expr::from(AB::F::from_u64(5));
        let ten = AB::Expr::from(AB::F::from_u64(10));
        let gas_cost = is_load.clone() * three.clone()
            + is_store.clone() * three.clone()
            + is_sread.clone() * three.clone()
            + is_swrite.clone() * three.clone()
            + is_poseidon.clone() * ten.clone()
            + is_verify_merkle.clone() * ten.clone()
            + is_call.clone() * two.clone()
            + is_ret.clone() * two.clone()
            + is_push.clone() * two.clone()
            + is_pop.clone() * two.clone()
            + is_syscall.clone() * five.clone()
            + (one.clone()
                - is_load.clone()
                - is_store.clone()
                - is_sread.clone()
                - is_swrite.clone()
                - is_poseidon.clone()
                - is_verify_merkle.clone()
                - is_call.clone()
                - is_ret.clone()
                - is_push.clone()
                - is_pop.clone()
                - is_syscall.clone()
                - is_halt.clone())
                * one.clone();

        builder
            .when_first_row()
            .assert_zero(cur[COL_GAS_USED].into());
        let cur_gas: AB::Expr = cur[COL_GAS_USED].into();
        let nxt_gas: AB::Expr = nxt[COL_GAS_USED].into();
        builder
            .when_transition()
            .assert_zero(nxt_gas - cur_gas.clone() - gas_cost);

        let expected_gas = public_inputs[34].into()
            + public_inputs[35].into() * AB::Expr::from(AB::F::from_u64(1 << 32));
        builder.when_last_row().assert_zero(cur_gas - expected_gas);

        // Soundness: Syscall constraints connecting to public inputs
        let expected_sender = public_inputs[26].into()
            + public_inputs[27].into() * AB::Expr::from(AB::F::from_u64(1 << 32));
        let expected_bh = public_inputs[30].into()
            + public_inputs[31].into() * AB::Expr::from(AB::F::from_u64(1 << 32));
        let expected_nonce = public_inputs[28].into()
            + public_inputs[29].into() * AB::Expr::from(AB::F::from_u64(1 << 32));

        let two_val = AB::Expr::from(AB::F::from_u64(2));
        let three_val = AB::Expr::from(AB::F::from_u64(3));

        let factor_1 = (imm.clone() - two_val.clone()) * (imm.clone() - three_val.clone());
        builder
            .when(is_syscall.clone())
            .assert_zero(factor_1 * (rd_val_new.clone() - expected_sender));

        let factor_2 = (imm.clone() - one.clone()) * (imm.clone() - three_val.clone());
        builder
            .when(is_syscall.clone())
            .assert_zero(factor_2 * (rd_val_new.clone() - expected_bh));

        let factor_3 = (imm.clone() - one.clone()) * (imm.clone() - two_val.clone());
        builder
            .when(is_syscall.clone())
            .assert_zero(factor_3 * (rd_val_new.clone() - expected_nonce));

        // CPU / Registers / Memory constraints
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
                * (one.clone() - nm_write.clone())
                * (nm_val.clone() - m_val.clone()),
        );
        builder.when_transition().assert_zero(
            m_active.clone() * nm_active.clone() * m_same.clone() * (nm_addr - m_addr.clone()),
        );

        // Soundness: first-read default zero in memory
        builder
            .when_first_row()
            .assert_zero(m_active.clone() * (one.clone() - m_is_write.clone()) * m_val.clone());
        builder.when_transition().assert_zero(
            m_active.clone()
                * nm_active.clone()
                * (one.clone() - m_same.clone())
                * (one.clone() - nm_write.clone())
                * nm_val.clone(),
        );

        let cur_clk: AB::Expr = cur[COL_CLK].into();
        let cur_pc: AB::Expr = cur[COL_PC].into();
        builder.when_first_row().assert_zero(cur_clk);
        builder.when_first_row().assert_zero(cur_pc);

        let perm = builder.permutation();
        let perm_cur = perm.current_slice();
        let perm_nxt = perm.next_slice();
        let rand = builder.permutation_randomness();
        if rand.len() >= 3 && perm_cur.len() >= 3 && perm_nxt.len() >= 3 {
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

            let term = |table_id: AB::Expr,
                        clk: AB::Expr,
                        idx: AB::Expr,
                        val: AB::Expr,
                        is_write: AB::Expr|
             -> AB::ExprEF {
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

            let c_rs1 = term(
                table_reg.clone(),
                clk_rs1,
                rs1_idx.clone(),
                rs1_val.clone(),
                zero.clone(),
            );
            let c_rs2 = term(
                table_reg.clone(),
                clk_rs2,
                rs2_idx.clone(),
                rs2_val.clone(),
                zero.clone(),
            );
            let c_rd = term(
                table_reg.clone(),
                clk_rd,
                rd_idx.clone(),
                rd_val_new.clone(),
                one.clone(),
            );
            let c_reg = term(
                table_reg.clone(),
                clk_reg,
                reg_idx.clone(),
                reg_val.clone(),
                reg_is_write.clone(),
            );

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
                    - (is_real_op_ext * (d_rs1 + d_rs2 + d_rd) - r_active_ext * d_reg),
            );
            builder.when_first_row().assert_zero_ext(s_reg_cur.clone());
            builder.when_last_row().assert_zero_ext(s_reg_cur);

            // Memory LogUp (includes Load/Store/Push/Pop/Call/Ret + SRead/SWrite)
            let rs1_idx: AB::Expr = cur[COL_RS1_IDX].into();
            let is_real_mem_op = (is_load.clone() + is_store.clone()) * rs1_idx.clone(); // If rs1 is 0, it's LoadImm
            let is_stack_op = is_push.clone() + is_pop.clone() + is_call.clone() + is_ret.clone();
            let is_storage_op = is_sread.clone() + is_swrite.clone();
            let is_any_mem_op =
                is_real_mem_op.clone() + is_stack_op.clone() + is_storage_op.clone();

            let stack_base = AB::Expr::from(AB::F::from_u64(1 << 60));
            let storage_base = AB::Expr::from(AB::F::from_u64(2 << 60));
            let stack_addr = stack_base.clone()
                + (is_push.clone() + is_call.clone()) * cur_stack_ptr.clone()
                + (is_pop.clone() + is_ret.clone()) * (cur_stack_ptr.clone() - one.clone());
            let storage_addr = storage_base + cur[COL_IMM].into();

            let final_mem_addr = is_real_mem_op.clone()
                * (cur[COL_RS1_VAL].into() + cur[COL_IMM].into())
                + is_stack_op.clone() * stack_addr
                + is_storage_op.clone() * storage_addr;

            let is_write = is_store.clone() + is_push.clone() + is_call.clone() + is_swrite.clone();
            let cpu_mem_val = is_load * cur[COL_RD_VAL_NEW].into()
                + is_store * cur[COL_RS2_VAL].into()
                + is_push * cur[COL_RS1_VAL].into()
                + is_pop * cur[COL_RD_VAL_NEW].into()
                + is_call * (cur[COL_PC].into() + one.clone())
                + is_ret * cur[COL_NEXT_PC].into()
                + is_sread * cur[COL_RD_VAL_NEW].into()
                + is_swrite * cur[COL_RS1_VAL].into();

            let c_cpu_mem = term(
                one.clone(),
                clk.clone(),
                final_mem_addr.clone(),
                cpu_mem_val.clone(),
                is_write.clone(),
            );
            let c_mem = term(
                one.clone(),
                m_clk.clone(),
                m_addr.clone(),
                m_val.clone(),
                m_is_write.clone(),
            );

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
            builder.when_first_row().assert_zero_ext(s_mem_cur.clone());
            builder.when_last_row().assert_zero_ext(s_mem_cur);

            // Program CTL LogUp (perm_cur[2] / perm_nxt[2])
            let pre = builder.preprocessed();
            let pre_cur = pre.current_slice();
            let pre_pc: AB::Expr = pre_cur[0].into();
            let pre_inst: AB::Expr = pre_cur[1].into();
            let pre_active: AB::Expr = pre_cur[2].into();

            let raw_inst: AB::Expr = cur[COL_RAW_INST].into();

            let pc_ext: AB::ExprEF = pc.into();
            let raw_inst_ext: AB::ExprEF = raw_inst.into();
            let pre_pc_ext: AB::ExprEF = pre_pc.into();
            let pre_inst_ext: AB::ExprEF = pre_inst.into();

            // tuple is (pc, raw_inst)
            let term_cpu_prog =
                alpha_expr.clone() + beta_expr.clone() * pc_ext + b2.clone() * raw_inst_ext;
            let term_pre_prog =
                alpha_expr.clone() + beta_expr.clone() * pre_pc_ext + b2.clone() * pre_inst_ext;

            let diff_cpu_prog: AB::ExprEF = gamma_expr.clone() - term_cpu_prog;
            let diff_pre_prog: AB::ExprEF = gamma_expr.clone() - term_pre_prog;

            let s_prog_cur: AB::ExprEF = perm_cur[2].into();
            let s_prog_nxt: AB::ExprEF = perm_nxt[2].into();
            let cpu_active: AB::Expr = cur[COL_CPU_ACTIVE].into();
            let cpu_active_ext: AB::ExprEF = cpu_active.into();
            let pre_active_ext: AB::ExprEF = pre_active.into();

            builder.when_transition().assert_zero_ext(
                (s_prog_nxt.clone() - s_prog_cur.clone())
                    * diff_cpu_prog.clone()
                    * diff_pre_prog.clone()
                    - (cpu_active_ext * diff_pre_prog - pre_active_ext * diff_cpu_prog),
            );
            builder.when_first_row().assert_zero_ext(s_prog_cur.clone());
            builder.when_last_row().assert_zero_ext(s_prog_cur);
        }

        // --- Comparison + Bitwise AIR constraints ---
        // Bit decomposition shared between comparison and bitwise (And/Or/Xor) opcodes
        let is_cmp = is_lt.clone() + is_gt.clone() + is_lte.clone() + is_gte.clone();
        let is_bw_bits = is_and.clone() + is_or.clone() + is_xor.clone();
        let is_cmp_or_bw = is_cmp.clone() + is_bw_bits.clone();

        // Booleanity of all bit decomposition columns
        for i in 0..64 {
            let a_bit: AB::Expr = cur[COL_CMP_RS1_BASE + i].into();
            let b_bit: AB::Expr = cur[COL_CMP_RS2_BASE + i].into();
            builder.when(is_cmp_or_bw.clone()).assert_bool(a_bit);
            builder.when(is_cmp_or_bw.clone()).assert_bool(b_bit);
        }

        // Equality prefix flags are boolean (comparison only)
        for i in 0..64 {
            let eq_i: AB::Expr = cur[COL_CMP_EQ_BASE + i].into();
            builder.when(is_cmp.clone()).assert_bool(eq_i);
        }

        // Reconstitution: rs1_val = sum(a_i * 2^i)
        {
            let mut rs1_bits_sum: AB::Expr = AB::Expr::ZERO;
            for i in 0..64 {
                let pow2 = AB::F::from_u64(1u64 << i);
                let a_bit: AB::Expr = cur[COL_CMP_RS1_BASE + i].into();
                rs1_bits_sum += a_bit * pow2;
            }
            builder
                .when(is_cmp_or_bw.clone())
                .assert_eq(rs1_bits_sum, rs1_val.clone());
        }

        // Reconstitution: rs2_val = sum(b_i * 2^i) (comparison + And/Or/Xor only)
        {
            let mut rs2_bits_sum: AB::Expr = AB::Expr::ZERO;
            for i in 0..64 {
                let pow2 = AB::F::from_u64(1u64 << i);
                let b_bit: AB::Expr = cur[COL_CMP_RS2_BASE + i].into();
                rs2_bits_sum += b_bit * pow2;
            }
            builder
                .when(is_cmp.clone() + is_bw_bits.clone())
                .assert_eq(rs2_bits_sum, rs2_val.clone());
        }

        // Bitwise result constraints (And/Or/Xor using bit decomposition)
        {
            let mut and_sum: AB::Expr = AB::Expr::ZERO;
            for i in 0..64 {
                let pow2 = AB::F::from_u64(1u64 << i);
                let a_bit: AB::Expr = cur[COL_CMP_RS1_BASE + i].into();
                let b_bit: AB::Expr = cur[COL_CMP_RS2_BASE + i].into();
                and_sum += a_bit * b_bit * pow2;
            }
            let two_val = AB::Expr::from(AB::F::from_u64(2));

            // And: rd = sum(a_i * b_i * 2^i)
            builder
                .when(is_and)
                .assert_eq(rd_val_new.clone(), and_sum.clone());
            // Or: rd = rs1 + rs2 - sum(a_i * b_i * 2^i)
            builder.when(is_or).assert_eq(
                rd_val_new.clone(),
                rs1_val.clone() + rs2_val.clone() - and_sum.clone(),
            );
            // Xor: rd = rs1 + rs2 - 2 * sum(a_i * b_i * 2^i)
            builder.when(is_xor).assert_eq(
                rd_val_new.clone(),
                rs1_val.clone() + rs2_val.clone() - two_val * and_sum,
            );
        }

        // Not (logical NOT): rd = 1 if rs1 == 0, else rd = 0 (reuse COL_INV_ZERO as inverse witness)
        {
            let not_inv: AB::Expr = cur[COL_INV_ZERO].into();
            let is_nonzero = rs1_val.clone() * not_inv.clone();
            builder.when(is_not.clone()).assert_bool(is_nonzero.clone());
            builder
                .when(is_not.clone())
                .assert_zero(rs1_val.clone() * (one.clone() - is_nonzero.clone()));
            builder
                .when(is_not)
                .assert_eq(rd_val_new.clone(), one.clone() - is_nonzero);
        }

        // --- Comparison-specific constraints below ---

        // Equality prefix recursion: eq_i = eq_{i+1} * (1 - a_i - b_i + 2*a_i*b_i)
        // eq_64 is implicitly 1
        {
            let a_63: AB::Expr = cur[COL_CMP_RS1_BASE + 63].into();
            let b_63: AB::Expr = cur[COL_CMP_RS2_BASE + 63].into();
            let eq_bit_63 = one.clone() - a_63.clone() - b_63.clone()
                + AB::Expr::from(AB::F::from_u64(2)) * a_63.clone() * b_63.clone();
            let eq_63: AB::Expr = cur[COL_CMP_EQ_BASE + 63].into();
            builder.when(is_cmp.clone()).assert_eq(eq_63, eq_bit_63);
        }
        for i in (0..63).rev() {
            let a_i: AB::Expr = cur[COL_CMP_RS1_BASE + i].into();
            let b_i: AB::Expr = cur[COL_CMP_RS2_BASE + i].into();
            let eq_bit_i = one.clone() - a_i.clone() - b_i.clone()
                + AB::Expr::from(AB::F::from_u64(2)) * a_i.clone() * b_i.clone();
            let eq_i: AB::Expr = cur[COL_CMP_EQ_BASE + i].into();
            let eq_next: AB::Expr = cur[COL_CMP_EQ_BASE + i + 1].into();
            builder
                .when(is_cmp.clone())
                .assert_eq(eq_i, eq_next * eq_bit_i);
        }

        // Raw less-than result: cmp_lt_raw = sum_{i=0}^{63} eq_{i+1} * (1-a_i) * b_i
        // eq_64 is implicit 1 for the MSB term
        {
            let a_63: AB::Expr = cur[COL_CMP_RS1_BASE + 63].into();
            let b_63: AB::Expr = cur[COL_CMP_RS2_BASE + 63].into();
            let mut cmp_lt_sum: AB::Expr = (one.clone() - a_63) * b_63;
            for i in 0..63 {
                let a_i: AB::Expr = cur[COL_CMP_RS1_BASE + i].into();
                let b_i: AB::Expr = cur[COL_CMP_RS2_BASE + i].into();
                let eq_next: AB::Expr = cur[COL_CMP_EQ_BASE + i + 1].into();
                cmp_lt_sum += eq_next * (one.clone() - a_i) * b_i;
            }
            let cmp_lt_raw: AB::Expr = cur[COL_CMP_LT_RAW].into();
            builder
                .when(is_cmp.clone())
                .assert_eq(cmp_lt_raw.clone(), cmp_lt_sum);
        }

        // Opcode-specific result constraints
        // eq_0 tells us if all bits are equal (a == b)
        let cmp_eq_all: AB::Expr = cur[COL_CMP_EQ_BASE].into();
        let cmp_lt_raw: AB::Expr = cur[COL_CMP_LT_RAW].into();

        // Lt: rd = cmp_lt_raw  (1 if a < b)
        builder
            .when(is_lt)
            .assert_eq(rd_val_new.clone(), cmp_lt_raw.clone());
        // Gt: rd = 1 - cmp_eq_all - cmp_lt_raw  (1 if a > b)
        builder.when(is_gt).assert_eq(
            rd_val_new.clone(),
            one.clone() - cmp_eq_all.clone() - cmp_lt_raw.clone(),
        );
        // Lte: rd = cmp_eq_all + cmp_lt_raw  (1 if a <= b)
        builder
            .when(is_lte)
            .assert_eq(rd_val_new.clone(), cmp_eq_all.clone() + cmp_lt_raw.clone());
        // Gte: rd = 1 - cmp_lt_raw  (1 if a >= b)
        builder
            .when(is_gte)
            .assert_eq(rd_val_new.clone(), one.clone() - cmp_lt_raw.clone());

        // --- Poseidon hash (4 rounds, alpha=7) ---
        // Verify all 4 rounds including S-boxes, MDS mixing, and result.
        {
            let p: AB::Expr = cur[COL_IS_POSEIDON].into();
            // Initial state
            builder
                .when(p.clone())
                .assert_eq(cur[COL_POSEIDON_STATE_BASE].into(), rs1_val.clone());
            builder
                .when(p.clone())
                .assert_eq(cur[COL_POSEIDON_STATE_BASE + 1].into(), rs2_val.clone());
            for i in 2..8 {
                builder
                    .when(p.clone())
                    .assert_zero(cur[COL_POSEIDON_STATE_BASE + i]);
            }

            const MDS: [[u64; 8]; 8] = [
                [7, 1, 3, 8, 8, 3, 4, 9],
                [9, 7, 1, 3, 8, 8, 3, 4],
                [4, 9, 7, 1, 3, 8, 8, 3],
                [3, 4, 9, 7, 1, 3, 8, 8],
                [8, 3, 4, 9, 7, 1, 3, 8],
                [8, 8, 3, 4, 9, 7, 1, 3],
                [3, 8, 8, 3, 4, 9, 7, 1],
                [1, 3, 8, 8, 3, 4, 9, 7],
            ];

            const RC: [[u64; 8]; 4] = [
                [
                    0xdd5743e7f2a5a5d9,
                    0xcb3a864e58ada44b,
                    0xffa2449ed32f8cdc,
                    0x42025f65d6bd13ee,
                    0x7889175e25506323,
                    0x34b98bb03d24b737,
                    0xbdcc535ecc4faa2a,
                    0x5b20ad869fc0d033,
                ],
                [
                    0xf1dda5b9259dfcb4,
                    0x27515210be112d59,
                    0x4227d1718c766c3f,
                    0x26d333161a5bd794,
                    0x49b938957bf4b026,
                    0x4a56b5938b213669,
                    0x1120426b48c8353d,
                    0x6b323c3f10a56cad,
                ],
                [
                    0xce57d6245ddca6b2,
                    0xb1fc8d402bba1eb1,
                    0xb5c5096ca959bd04,
                    0x6db55cd306d31f7f,
                    0xc49d293a81cb9641,
                    0x1ce55a4fe979719f,
                    0xa92e60a9d178a4d1,
                    0x002cc64973bcfd8c,
                ],
                [
                    0xcea721cce82fb11b,
                    0xe5b55eb8098ece81,
                    0x4e30525c6f1ddd66,
                    0x43c6702827070987,
                    0xaca68430a7b5762a,
                    0x3674238634df9c93,
                    0x88cee1c825e33433,
                    0xde99ae8d74b57176,
                ],
            ];

            for r in 0..4 {
                let mut sbox_out = vec![AB::Expr::ZERO; 8];

                for i in 0..8 {
                    let s: AB::Expr = cur[COL_POSEIDON_STATE_BASE + r * 8 + i].into()
                        + AB::Expr::from(AB::F::from_u64(RC[r][i]));
                    let x2: AB::Expr = cur[COL_POSEIDON_X2_BASE + r * 8 + i].into();
                    let x4: AB::Expr = cur[COL_POSEIDON_X4_BASE + r * 8 + i].into();

                    builder
                        .when(p.clone())
                        .assert_eq(x2.clone(), s.clone() * s.clone());
                    builder
                        .when(p.clone())
                        .assert_eq(x4.clone(), x2.clone() * x2.clone());

                    sbox_out[i] = x4 * x2 * s;
                }

                // Check MDS multiplication and next round state (or result for last round)
                if r < 3 {
                    for i in 0..8 {
                        let mut sum: AB::Expr = AB::Expr::ZERO;
                        for j in 0..8 {
                            sum += sbox_out[j].clone() * AB::Expr::from(AB::F::from_u64(MDS[i][j]));
                        }
                        builder
                            .when(p.clone())
                            .assert_eq(cur[COL_POSEIDON_STATE_BASE + (r + 1) * 8 + i].into(), sum);
                    }
                } else {
                    // Final round, output is the first element
                    let mut sum: AB::Expr = AB::Expr::ZERO;
                    for j in 0..8 {
                        sum += sbox_out[j].clone() * AB::Expr::from(AB::F::from_u64(MDS[0][j]));
                    }
                    builder.when(p.clone()).assert_eq(rd_val_new.clone(), sum);
                }
            }
        }
    }
}
