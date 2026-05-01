use p3_air::symbolic::SymbolicAirBuilder;
use p3_air::Air;
use p3_commit::Pcs;
use p3_matrix::Matrix;
use tracing::debug_span;

use crate::bud_stark::{ProverConstraintFolder, StarkGenericConfig, Val};

/// Prover-side reusable data for preprocessed columns.
pub struct PreprocessedProverData<SC: StarkGenericConfig> {
    pub width: usize,
    pub degree_bits: usize,
    pub commitment: crate::bud_stark::Com<SC>,
    pub prover_data: crate::bud_stark::ProverData<SC>,
}

/// Verifier-side reusable data for preprocessed columns.
#[derive(Clone)]
pub struct PreprocessedVerifierKey<SC: StarkGenericConfig> {
    pub width: usize,
    pub degree_bits: usize,
    pub commitment: crate::bud_stark::Com<SC>,
}

pub fn setup_preprocessed<SC, A>(
    config: &SC,
    air: &A,
    degree_bits: usize,
) -> Option<(PreprocessedProverData<SC>, PreprocessedVerifierKey<SC>)>
where
    SC: StarkGenericConfig,
    A: Air<SymbolicAirBuilder<Val<SC>>> + for<'a> Air<ProverConstraintFolder<'a, SC>>,
{
    let pcs = config.pcs();
    let is_zk = config.is_zk() as usize;

    let init_degree = 1 << degree_bits;
    let degree = 1 << (degree_bits + is_zk);

    let preprocessed = air.preprocessed_trace()?;

    let width = preprocessed.width();
    if width == 0 {
        return None;
    }

    assert_eq!(
        preprocessed.height(),
        init_degree,
        "preprocessed trace height must equal trace degree"
    );

    let trace_domain = pcs.natural_domain_for_degree(degree);
    let (commitment, prover_data) = debug_span!("commit to preprocessed trace")
        .in_scope(|| pcs.commit_preprocessing([(trace_domain, preprocessed)]));

    let degree_bits = degree_bits + is_zk;
    let prover_data = PreprocessedProverData {
        width,
        degree_bits,
        commitment: commitment.clone(),
        prover_data,
    };
    let vk = PreprocessedVerifierKey {
        width,
        degree_bits,
        commitment,
    };
    Some((prover_data, vk))
}
