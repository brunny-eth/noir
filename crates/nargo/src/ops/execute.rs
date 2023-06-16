use acvm::pwg::{PartialWitnessGeneratorStatus, ACVM};
use acvm::PartialWitnessGenerator;
use acvm::{acir::circuit::Circuit, acir::native_types::WitnessMap};

use crate::NargoError;

pub fn execute_circuit<B: PartialWitnessGenerator + Default>(
    _backend: &B,
    circuit: Circuit,
    initial_witness: WitnessMap,
) -> Result<WitnessMap, NargoError> {
    let mut acvm = ACVM::new(B::default(), circuit.opcodes, initial_witness);
    let solver_status = acvm.solve()?;
    if matches!(solver_status, PartialWitnessGeneratorStatus::RequiresOracleData { .. }) {
        todo!("Add oracle support to nargo execute")
    }

    Ok(acvm.finalize())
}
