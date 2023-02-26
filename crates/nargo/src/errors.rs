use acvm::OpcodeResolutionError;
use noirc_abi::errors::AbiError;
use std::path::PathBuf;
use thiserror::Error;

use crate::cli::IOError;

#[derive(Debug, Error)]
pub(crate) enum CliError {
    #[error("{0}")]
    Generic(String),

    #[error("Failed to verify proof {}", .0.display())]
    InvalidProof(PathBuf),

    /// Error while compiling Noir into ACIR.
    #[error("Failed to compile circuit")]
    CompilationError,

    /// ABI encoding/decoding error
    #[error(transparent)]
    IOError(#[from] IOError),

    /// ABI encoding/decoding error
    #[error(transparent)]
    AbiError(#[from] AbiError),

    /// ACIR circuit solving error
    #[error(transparent)]
    SolvingError(#[from] OpcodeResolutionError),
}
