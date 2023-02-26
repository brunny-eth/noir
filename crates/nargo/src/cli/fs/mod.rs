use hex::FromHexError;
use noirc_abi::errors::InputParserError;
use std::{
    fs::File,
    io::Write,
    path::{Path, PathBuf},
};
use thiserror::Error;

pub(super) mod inputs;
pub(super) mod keys;
pub(super) mod program;
pub(super) mod proof;
pub(super) mod witness;

#[derive(Debug, Error)]
pub(crate) enum IOError {
    /// Error during IO
    #[error(transparent)]
    IO(#[from] std::io::Error),

    #[error("Error: {} is not a valid path\nRun either `nargo compile` to generate missing build artifacts or `nargo prove` to construct a proof", .0.display())]
    PathNotValid(PathBuf),

    #[error(
        " Error: cannot find {0}.toml file.\n Expected location: {1:?} \n Please generate this file at the expected location."
    )]
    MissingTomlFile(String, PathBuf),

    #[error("Error: the circuit you are trying to prove differs from the build artifact at {}\nYou must call `nargo compile` to generate the correct proving and verification keys for this circuit", .0.display())]
    MismatchedAcir(PathBuf),

    /// Attempted to parse invalid hex data.
    #[error("Error: could not parse hex build artifact (proof, proving and/or verification keys, ACIR checksum) ({0})")]
    HexArtifactNotValid(#[from] FromHexError),

    /// Input parsing error
    #[error(transparent)]
    InputParserError(#[from] InputParserError),
}

pub(super) fn create_dir(named_dir: &Path) -> Result<PathBuf, IOError> {
    std::fs::create_dir_all(named_dir)?;

    Ok(PathBuf::from(named_dir))
}

pub(super) fn write_to_file(bytes: &[u8], path: &Path) -> Result<(), IOError> {
    let mut file = File::create(path)?;

    file.write_all(bytes)?;

    Ok(())
}

pub(super) fn load_hex_data<P: AsRef<Path>>(path: P) -> Result<Vec<u8>, IOError> {
    let hex_data: Vec<_> =
        std::fs::read(&path).map_err(|_| IOError::PathNotValid(path.as_ref().to_path_buf()))?;

    let raw_bytes = hex::decode(hex_data)?;

    Ok(raw_bytes)
}
