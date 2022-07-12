// Copyright (c) zkMove Authors

use crate::move_circuit::FastMoveCircuit;
use error::{RuntimeError, StatusCode, VmResult};
use halo2_proofs::plonk::{
    create_proof, keygen_pk, keygen_vk, verify_proof, ProvingKey, SingleVerifier,
};
use halo2_proofs::poly::commitment::Params;
use halo2_proofs::transcript::{Blake2bRead, Blake2bWrite, Challenge255};
use halo2_proofs::{dev::MockProver, pasta::EqAffine, pasta::Fp};
use logger::prelude::*;
use move_binary_format::CompiledModule;
use movelang::argument::ScriptArguments;
use movelang::loader::MoveLoader;
use movelang::state::StateStore;
use rand_core::OsRng;

pub struct Runtime {
    loader: MoveLoader,
}

impl Runtime {
    pub fn new() -> Self {
        Runtime {
            loader: MoveLoader::new(),
        }
    }

    pub fn loader(&self) -> &MoveLoader {
        &self.loader
    }

    pub fn mock_prove_script(
        &self,
        script: Vec<u8>,
        modules: Vec<CompiledModule>,
        args: Option<ScriptArguments>,
        data_store: &mut StateStore,
        k: u32,
    ) -> VmResult<()> {
        let circuit = FastMoveCircuit::new(script, modules, args, data_store, self.loader());

        let public_inputs = vec![Fp::zero()];
        let prover = MockProver::<Fp>::run(k, &circuit, vec![public_inputs]).map_err(|e| {
            debug!("Prover Error: {:?}", e);
            RuntimeError::new(StatusCode::SynthesisError)
        })?;
        assert_eq!(prover.verify(), Ok(()));
        Ok(())
    }

    pub fn setup_script(
        &self,
        script: Vec<u8>,
        modules: Vec<CompiledModule>,
        data_store: &mut StateStore,
        params: &Params<EqAffine>,
    ) -> VmResult<ProvingKey<EqAffine>> {
        let circuit = FastMoveCircuit::new(script, modules, None, data_store, self.loader());
        debug!("Generate vk");
        let vk = keygen_vk(params, &circuit).map_err(|_| {
            RuntimeError::new(StatusCode::SynthesisError)
                .with_message("keygen_vk should not fail".to_string())
        })?;
        debug!("Generate pk");
        let pk = keygen_pk(params, vk, &circuit).map_err(|_| {
            RuntimeError::new(StatusCode::SynthesisError)
                .with_message("keygen_pk should not fail".to_string())
        })?;
        Ok(pk)
    }

    pub fn prove_script(
        &self,
        script: Vec<u8>,
        modules: Vec<CompiledModule>,
        args: Option<ScriptArguments>,
        data_store: &mut StateStore,
        params: &Params<EqAffine>,
        pk: ProvingKey<EqAffine>,
    ) -> VmResult<()> {
        let circuit = FastMoveCircuit::new(script, modules, args, data_store, self.loader());

        let public_inputs = vec![Fp::zero()];
        let mut transcript = Blake2bWrite::<_, _, Challenge255<_>>::init(vec![]);
        // Create a proof
        create_proof(
            params,
            &pk,
            &[circuit],
            &[&[public_inputs.as_slice()]],
            OsRng,
            &mut transcript,
        )
        .expect("proof generation should not fail");
        let proof: Vec<u8> = transcript.finalize();

        let strategy = SingleVerifier::new(&params);
        let mut transcript = Blake2bRead::<_, _, Challenge255<_>>::init(&proof[..]);
        let result = verify_proof(
            params,
            pk.get_vk(),
            strategy,
            &[&[public_inputs.as_slice()]],
            &mut transcript,
        );
        assert!(result.is_ok());
        Ok(())
    }
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new()
    }
}
