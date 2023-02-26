use std::path::{Path, PathBuf};

use acvm::{acir::circuit::Circuit, hash_constraint_system};
use noirc_driver::CompiledProgram;

use crate::{constants::ACIR_CHECKSUM, errors::CliError};

use super::{create_dir, write_to_file, IOError};

pub(crate) fn save_program_to_file<P: AsRef<Path>>(
    compiled_program: &CompiledProgram,
    circuit_name: &str,
    circuit_dir: P,
) -> Result<PathBuf, IOError> {
    create_dir(circuit_dir.as_ref())?;
    let circuit_path = circuit_dir.as_ref().join(circuit_name).with_extension("json");

    write_to_file(&serde_json::to_vec(compiled_program).unwrap(), &circuit_path)?;

    Ok(circuit_path)
}

pub(crate) fn save_acir_hash_to_dir<P: AsRef<Path>>(
    circuit: &Circuit,
    hash_name: &str,
    hash_dir: P,
) -> Result<PathBuf, IOError> {
    let acir_hash = hash_constraint_system(circuit);
    let hash_path = hash_dir.as_ref().join(hash_name).with_extension(ACIR_CHECKSUM);
    write_to_file(hex::encode(acir_hash).as_bytes(), &hash_path)?;

    Ok(hash_path)
}

pub(crate) fn read_program_from_file<P: AsRef<Path>>(
    circuit_path: P,
) -> Result<CompiledProgram, CliError> {
    let file_path = circuit_path.as_ref().with_extension("json");

    let input_string = std::fs::read(&file_path).map_err(|_| IOError::PathNotValid(file_path))?;

    let program = serde_json::from_slice(&input_string).expect("could not deserialize program");

    Ok(program)
}
