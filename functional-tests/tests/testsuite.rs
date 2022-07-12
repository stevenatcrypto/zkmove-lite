// Copyright (c) zkMove Authors

use anyhow::Result;
use halo2_proofs::pasta::EqAffine;
use halo2_proofs::poly::commitment::Params;
use logger::prelude::*;
use movelang::state::StateStore;
use movelang::{argument::ScriptArguments, compiler::compile_script};
use std::fs::File;
use std::io::Read;
use std::path::Path;
use vm::runtime::Runtime;

pub const TEST_MODULE_PATH: &str = "tests/modules";

#[derive(Debug)]
struct RunConfig {
    args: Option<ScriptArguments>,
    modules: Vec<String>,
}

fn parse_config(script_file: &Path) -> Result<RunConfig> {
    let mut config = RunConfig {
        args: None,
        modules: vec![],
    };
    let file_str = script_file.to_str().expect("path is None.");

    let mut f = File::open(script_file)
        .map_err(|err| std::io::Error::new(err.kind(), format!("{}: {}", err, file_str)))?;
    let mut buffer = String::new();
    f.read_to_string(&mut buffer)?;

    for line in buffer.lines().into_iter() {
        let s = line.split_whitespace().collect::<String>();
        if let Some(s) = s.strip_prefix("//!args:") {
            config.args = Some(s.parse::<ScriptArguments>()?);
        }
        if let Some(s) = s.strip_prefix("//!mods:") {
            config.modules.push(s.to_string()); //todo: support multiple modules
        }
    }
    Ok(config)
}

fn vm_test(path: &Path) -> datatest_stable::Result<()> {
    logger::init_for_test();
    let script_file = path.to_str().expect("path is None.");
    debug!("Run test {:?}", script_file);

    let mut targets = vec![];
    targets.push(script_file.to_string());
    let config = parse_config(path)?;
    for module in config.modules.into_iter() {
        let path = Path::new(TEST_MODULE_PATH)
            .join(module)
            .to_str()
            .unwrap()
            .to_string();
        targets.push(path);
    }
    debug!(
        "script arguments {:?}, compile targets {:?}",
        config.args, targets
    );

    let (compiled_script, compiled_modules) = compile_script(targets)?;

    if let Some(script) = compiled_script {
        let mut script_bytes = vec![];
        script.serialize(&mut script_bytes)?;

        let k = 6;
        let runtime = Runtime::new();
        let mut state = StateStore::new();
        for module in compiled_modules.clone().into_iter() {
            state.add_module(module);
        }

        debug!(
            "Generate zk proof for script {:?} with mock prover",
            script_file
        );
        runtime.mock_prove_script(
            script_bytes.clone(),
            compiled_modules.clone(),
            config.args.clone(),
            &mut state,
            k,
        )?;

        debug!("Generate parameters for script {:?}", script_file);
        let params: Params<EqAffine> = Params::new(k);
        let pk = runtime.setup_script(
            script_bytes.clone(),
            compiled_modules.clone(),
            &mut state,
            &params,
        )?;

        debug!(
            "Generate zk proof for script {:?} with real prover",
            script_file
        );
        runtime.prove_script(
            script_bytes.clone(),
            compiled_modules.clone(),
            config.args.clone(),
            &mut state,
            &params,
            pk,
        )?;

        debug!("Generate execution trace for script {:?}", script_file);
        // runtime.generate_trace::<Fp>(script_bytes, compiled_modules, config.args, &mut state)?;
    }

    Ok(())
}

datatest_stable::harness!(vm_test, "tests/scripts", r".*\.move");
