use std::path::{Path, PathBuf};

use acvm::acir::native_types::Witness;
use noirc_abi::WitnessMap;

use super::{create_dir, write_to_file, IOError};
use crate::constants::WITNESS_EXT;

pub(crate) fn save_witness_to_dir<P: AsRef<Path>>(
    witness: WitnessMap,
    witness_name: &str,
    witness_dir: P,
) -> Result<PathBuf, IOError> {
    create_dir(witness_dir.as_ref())?;
    let witness_path = witness_dir.as_ref().join(witness_name).with_extension(WITNESS_EXT);

    let buf = Witness::to_bytes(&witness);

    write_to_file(buf.as_slice(), &witness_path)?;

    Ok(witness_path)
}
