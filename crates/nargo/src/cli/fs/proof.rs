use std::path::{Path, PathBuf};

use crate::constants::PROOF_EXT;

use super::{create_dir, write_to_file, IOError};

pub(crate) fn save_proof_to_dir<P: AsRef<Path>>(
    proof: &[u8],
    proof_name: &str,
    proof_dir: P,
) -> Result<PathBuf, IOError> {
    create_dir(proof_dir.as_ref())?;
    let proof_path = proof_dir.as_ref().join(proof_name).with_extension(PROOF_EXT);

    write_to_file(hex::encode(proof).as_bytes(), &proof_path)?;

    Ok(proof_path)
}
