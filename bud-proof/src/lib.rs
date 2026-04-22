pub mod adapter;
pub mod winterfell_prover;
pub mod plonky3_air;
pub mod plonky3_prover;

pub use adapter::{ProverAdapter, Proof};
pub use plonky3_prover::Plonky3Adapter;
pub use plonky3_prover::Plonky3Adapter as DefaultAdapter;
