use p3_air::{Air, AirBuilder, BaseAir, RowWindow};
use p3_field::Field;
use p3_matrix::Matrix;

/// The columns of the memory trace matrix.
pub const COL_MEM_CLK: usize = 0;
pub const COL_MEM_ADDR: usize = 1;
pub const COL_MEM_VAL: usize = 2;
pub const COL_MEM_IS_WRITE: usize = 3;
pub const COL_MEM_ACTIVE: usize = 4;
pub const COL_MEM_SAME: usize = 5;

pub const MEM_TRACE_WIDTH: usize = 6;

/// A dedicated AIR for memory permutation checks.
/// Currently, BudZKVM evaluates constraints as a monolithic STARK in `plonky3_air.rs`.
/// This struct is provided to support future migration to a multi-STARK architecture
/// using Cross-Table Lookups (CTL) for memory integrity.
pub struct MemoryAir {
    pub num_steps: usize,
}

impl<F: Field> BaseAir<F> for MemoryAir {
    fn width(&self) -> usize {
        MEM_TRACE_WIDTH
    }
}

impl<AB: AirBuilder> Air<AB> for MemoryAir {
    fn eval(&self, builder: &mut AB) {
        let main = builder.main();
        let local = main.row_slice(0);
        let next = main.row_slice(1);

        let m_val: AB::Expr = local[COL_MEM_VAL].into();
        let m_active: AB::Expr = local[COL_MEM_ACTIVE].into();
        let m_same: AB::Expr = local[COL_MEM_SAME].into();

        let nm_val: AB::Expr = next[COL_MEM_VAL].into();
        let nm_active: AB::Expr = next[COL_MEM_ACTIVE].into();
        let nm_write: AB::Expr = next[COL_MEM_IS_WRITE].into();
        let m_addr: AB::Expr = local[COL_MEM_ADDR].into();
        let nm_addr: AB::Expr = next[COL_MEM_ADDR].into();

        let one = AB::Expr::from(AB::F::ONE);

        // Memory permutation constraint 1:
        // If the same address is accessed consecutively and the next access is a READ (is_write == 0),
        // the value read must equal the last value stored or read.
        builder.when_transition().assert_zero(
            m_active.clone()
                * nm_active.clone()
                * m_same.clone()
                * (one.clone() - nm_write)
                * (nm_val - m_val),
        );

        // Memory permutation constraint 2:
        // The m_same flag asserts that the address remains the same across transition.
        builder.when_transition().assert_zero(
            m_active.clone() * nm_active.clone() * m_same.clone() * (nm_addr - m_addr),
        );
        
        // Active flags must be boolean
        builder.assert_bool(m_active.clone());
    }
}
