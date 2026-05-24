use bud_vm::Step;
use serde::{Deserialize, Serialize};
use tiny_keccak::{Hasher, Keccak};

#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct ExecutionPublicInputs {
    pub chain_id: u64,
    pub program_hash: [u8; 32],
    pub initial_state_root: [u8; 32],
    pub final_state_root: [u8; 32],
    pub sender: u64,
    pub nonce: u64,
    pub block_height: u64,
    pub gas_limit: u64,
    pub gas_used: u64,
    pub exit_code: u64,
    pub trace_len: u64,
    pub event_digest: [u8; 32],
}

impl ExecutionPublicInputs {
    pub fn to_canonical_bytes(&self) -> Vec<u8> {
        let mut bytes = Vec::with_capacity(176);
        bytes.extend_from_slice(&self.chain_id.to_le_bytes());
        bytes.extend_from_slice(&self.program_hash);
        bytes.extend_from_slice(&self.initial_state_root);
        bytes.extend_from_slice(&self.final_state_root);
        bytes.extend_from_slice(&self.sender.to_le_bytes());
        bytes.extend_from_slice(&self.nonce.to_le_bytes());
        bytes.extend_from_slice(&self.block_height.to_le_bytes());
        bytes.extend_from_slice(&self.gas_limit.to_le_bytes());
        bytes.extend_from_slice(&self.gas_used.to_le_bytes());
        bytes.extend_from_slice(&self.exit_code.to_le_bytes());
        bytes.extend_from_slice(&self.trace_len.to_le_bytes());
        bytes.extend_from_slice(&self.event_digest);
        bytes
    }

    pub fn hash(&self) -> [u8; 32] {
        let bytes = self.to_canonical_bytes();
        let mut hasher = Keccak::v256();
        hasher.update(&bytes);
        let mut res = [0u8; 32];
        hasher.finalize(&mut res);
        res
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct ProofEnvelope {
    pub proof_format_version: u32,
    pub backend: String,
    pub p3_version: String,
    pub fri_params_id: String,
    pub public_inputs_hash: [u8; 32],
    pub proof_bytes: Vec<u8>,
    pub degree_bits: u32,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum ProverError {
    TraceGenerationError(String),
    ProverInternalError(String),
    SerializationError(String),
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub enum VerifyError {
    DeserializationError(String),
    InvalidEnvelope(String),
    PublicInputsMismatch,
    InvalidProof,
}

pub trait ProverAdapter {
    fn prove(
        trace: &[Step],
        public_inputs: &ExecutionPublicInputs,
        program: &[u64],
    ) -> Result<ProofEnvelope, ProverError>;

    fn verify(
        envelope: &ProofEnvelope,
        expected_inputs: &ExecutionPublicInputs,
        program: &[u64],
    ) -> Result<(), VerifyError>;
}
