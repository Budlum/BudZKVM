use std::vec::Vec;

use serde::{Deserialize, Serialize};

use crate::bud_stark::StarkGenericConfig;

#[derive(Clone, Serialize, Deserialize)]
#[serde(bound(
    serialize = "crate::bud_stark::Com<SC>: Serialize, SC::Challenge: Serialize, crate::bud_stark::PcsProof<SC>: Serialize",
    deserialize = "crate::bud_stark::Com<SC>: Deserialize<'de>, SC::Challenge: Deserialize<'de>, crate::bud_stark::PcsProof<SC>: Deserialize<'de>"
))]
pub struct Proof<SC: StarkGenericConfig> {
    pub commitments: Commitments<crate::bud_stark::Com<SC>>,
    pub opened_values: OpenedValues<SC::Challenge>,
    pub opening_proof: crate::bud_stark::PcsProof<SC>,
    pub degree_bits: usize,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Commitments<Com> {
    pub trace: Com,
    pub aux_trace: Option<Com>,
    pub quotient_chunks: Com,
    pub random: Option<Com>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OpenedValues<Challenge> {
    pub trace_local: Vec<Challenge>,
    pub trace_next: Option<Vec<Challenge>>,
    pub aux_trace_local: Option<Vec<Challenge>>,
    pub aux_trace_next: Option<Vec<Challenge>>,
    pub preprocessed_local: Option<Vec<Challenge>>,
    pub preprocessed_next: Option<Vec<Challenge>>,
    pub quotient_chunks: Vec<Vec<Challenge>>,
    pub random: Option<Vec<Challenge>>,
}
