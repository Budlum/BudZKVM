use serde::{Serialize, Deserialize};
use bud_vm::Step;

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Proof {
    pub data: Vec<u8>,
}

pub trait ProverAdapter {
    fn prove(trace: &[Step], num_steps: usize) -> Proof;
    fn verify(proof: &Proof, num_steps: usize) -> bool;
}
