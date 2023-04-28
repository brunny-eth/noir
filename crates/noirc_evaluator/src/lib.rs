#![forbid(unsafe_code)]
#![warn(unused_crate_dependencies, unused_extern_crates)]
#![warn(unreachable_pub)]
#![warn(clippy::semicolon_if_nothing_returned)]

mod brillig;
mod errors;
mod ssa;

// SSA code to create the SSA based IR
// for functions and execute different optimizations.
pub mod ssa_refactor;
// Frontend helper module to translate a different AST
// into the SSA IR.
pub mod frontend;

use acvm::{
    acir::circuit::{opcodes::Opcode as AcirOpcode, Circuit, PublicInputs},
    acir::native_types::{Expression, Witness},
    compiler::transformers::IsOpcodeSupported,
    Language,
};
use errors::{RuntimeError, RuntimeErrorKind};
use iter_extended::btree_map;
use noirc_abi::{Abi, AbiType, AbiVisibility};
use noirc_frontend::monomorphization::ast::*;
use ssa::{node::ObjectType, ssa_gen::IrGenerator};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Default)]
pub struct Evaluator {
    // Why is this not u64?
    //
    // At the moment, wasm32 is being used in the default backend
    // so it is safer to use a u32, at least until clang is changed
    // to compile wasm64.
    //
    // XXX: Barretenberg, reserves the first index to have value 0.
    // When we increment, we do not use this index at all.
    // This means that every constraint system at the moment, will either need
    // to decrease each index by 1, or create a dummy witness.
    //
    // We ideally want to not have this and have Barretenberg apply the
    // following transformation to the witness index : f(i) = i + 1
    current_witness_index: u32,
    // This is the number of witnesses indices used when
    // creating the private/public inputs of the ABI.
    num_witnesses_abi_len: usize,
    param_witnesses: BTreeMap<String, Vec<Witness>>,
    // This is the list of witness indices which are linked to public parameters.
    // Witnesses below `num_witnesses_abi_len` and not included in this set
    // correspond to private parameters and must not be made public.
    public_parameters: BTreeSet<Witness>,
    // The witness indices for return values are not guaranteed to be contiguous
    // and increasing as for `public_parameters`. We then use a `Vec` rather
    // than a `BTreeSet` to preserve this order for the ABI.
    return_values: Vec<Witness>,

    opcodes: Vec<AcirOpcode>,
}

/// Compiles the Program into ACIR and applies optimizations to the arithmetic gates
// XXX: We return the num_witnesses, but this is the max number of witnesses
// Some of these could have been removed due to optimizations. We need this number because the
// Standard format requires the number of witnesses. The max number is also fine.
// If we had a composer object, we would not need it
pub fn create_circuit(
    program: Program,
    np_language: Language,
    is_opcode_supported: IsOpcodeSupported,
    enable_logging: bool,
    show_output: bool,
) -> Result<(Circuit, Abi), RuntimeError> {
    let mut evaluator = Evaluator::default();

    // First evaluate the main function
    evaluator.evaluate_main_alt(program.clone(), enable_logging, show_output)?;

    let Evaluator {
        current_witness_index,
        param_witnesses,
        public_parameters,
        return_values,
        opcodes,
        ..
    } = evaluator;
    let optimized_circuit = acvm::compiler::compile(
        Circuit {
            current_witness_index,
            opcodes,
            public_parameters: PublicInputs(public_parameters),
            return_values: PublicInputs(return_values.iter().copied().collect()),
        },
        np_language,
        is_opcode_supported,
    )
    .map_err(|_| RuntimeErrorKind::Spanless(String::from("produced an acvm compile error")))?;

    let (parameters, return_type) = program.main_function_signature;
    let abi = Abi { parameters, param_witnesses, return_type, return_witnesses: return_values };

    Ok((optimized_circuit, abi))
}

impl Evaluator {
    // Returns true if the `witness_index`
    // was created in the ABI as a private input.
    //
    // Note: This method is used so that we don't convert private
    // ABI inputs into public outputs.
    fn is_private_abi_input(&self, witness_index: Witness) -> bool {
        // If the `witness_index` is more than the `num_witnesses_abi_len`
        // then it was created after the ABI was processed and is therefore
        // an intermediate variable.
        let is_intermediate_variable = witness_index.as_usize() > self.num_witnesses_abi_len;

        let is_public_input = self.public_parameters.contains(&witness_index);

        !is_intermediate_variable && !is_public_input
    }

    // Creates a new Witness index
    fn add_witness_to_cs(&mut self) -> Witness {
        self.current_witness_index += 1;
        Witness(self.current_witness_index)
    }

    pub fn current_witness_index(&self) -> u32 {
        self.current_witness_index
    }

    pub fn push_opcode(&mut self, gate: AcirOpcode) {
        self.opcodes.push(gate);
    }

    /// Compiles the AST into the intermediate format by evaluating the main function
    pub fn evaluate_main_alt(
        &mut self,
        program: Program,
        enable_logging: bool,
        show_output: bool,
    ) -> Result<(), RuntimeError> {
        let mut ir_gen = IrGenerator::new(program);
        self.parse_abi_alt(&mut ir_gen);

        // Now call the main function
        ir_gen.ssa_gen_main()?;

        //Generates ACIR representation:
        ir_gen.context.ir_to_acir(self, enable_logging, show_output)?;
        Ok(())
    }

    // When we are multiplying arithmetic gates by each other, if one gate has too many terms
    // It is better to create an intermediate variable which links to the gate and then multiply by that intermediate variable
    // instead
    pub fn create_intermediate_variable(&mut self, arithmetic_gate: Expression) -> Witness {
        // Create a unique witness name and add witness to the constraint system
        let inter_var_witness = self.add_witness_to_cs();

        // Link that witness to the arithmetic gate
        let constraint = &arithmetic_gate - &inter_var_witness;
        self.opcodes.push(AcirOpcode::Arithmetic(constraint));
        inter_var_witness
    }

    fn param_to_var(
        &mut self,
        name: &str,
        def: Definition,
        param_type: &AbiType,
        param_visibility: &AbiVisibility,
        ir_gen: &mut IrGenerator,
    ) -> Result<(), RuntimeErrorKind> {
        let witnesses = match param_type {
            AbiType::Field => {
                let witness = self.add_witness_to_cs();
                ir_gen.create_new_variable(
                    name.to_owned(),
                    Some(def),
                    ObjectType::native_field(),
                    Some(witness),
                );
                vec![witness]
            }
            AbiType::Array { length, typ } => {
                let witnesses = self.generate_array_witnesses(length, typ)?;

                ir_gen.abi_array(name, Some(def), typ.as_ref(), *length, &witnesses);
                witnesses
            }
            AbiType::Integer { sign: _, width } => {
                let witness = self.add_witness_to_cs();
                ssa::acir_gen::range_constraint(witness, *width, self)?;
                let obj_type = ir_gen.get_object_type_from_abi(param_type); // Fetch signedness of the integer
                ir_gen.create_new_variable(name.to_owned(), Some(def), obj_type, Some(witness));

                vec![witness]
            }
            AbiType::Boolean => {
                let witness = self.add_witness_to_cs();
                ssa::acir_gen::range_constraint(witness, 1, self)?;
                let obj_type = ObjectType::boolean();
                ir_gen.create_new_variable(name.to_owned(), Some(def), obj_type, Some(witness));

                vec![witness]
            }
            AbiType::Struct { fields } => {
                let new_fields = btree_map(fields, |(inner_name, value)| {
                    let new_name = format!("{name}.{inner_name}");
                    (new_name, value.clone())
                });

                let mut struct_witnesses: BTreeMap<String, Vec<Witness>> = BTreeMap::new();
                self.generate_struct_witnesses(&mut struct_witnesses, &new_fields)?;

                ir_gen.abi_struct(name, Some(def), fields, &struct_witnesses);
                struct_witnesses.values().flatten().copied().collect()
            }
            AbiType::String { length } => {
                let typ = AbiType::Integer { sign: noirc_abi::Sign::Unsigned, width: 8 };
                let witnesses = self.generate_array_witnesses(length, &typ)?;
                ir_gen.abi_array(name, Some(def), &typ, *length, &witnesses);
                witnesses
            }
        };

        if param_visibility == &AbiVisibility::Public {
            self.public_parameters.extend(witnesses.clone());
        }
        self.param_witnesses.insert(name.to_owned(), witnesses);

        Ok(())
    }

    fn generate_struct_witnesses(
        &mut self,
        struct_witnesses: &mut BTreeMap<String, Vec<Witness>>,
        fields: &BTreeMap<String, AbiType>,
    ) -> Result<(), RuntimeErrorKind> {
        for (name, typ) in fields {
            match typ {
                AbiType::Integer { width, .. } => {
                    let witness = self.add_witness_to_cs();
                    struct_witnesses.insert(name.clone(), vec![witness]);
                    ssa::acir_gen::range_constraint(witness, *width, self)?;
                }
                AbiType::Boolean => {
                    let witness = self.add_witness_to_cs();
                    struct_witnesses.insert(name.clone(), vec![witness]);
                    ssa::acir_gen::range_constraint(witness, 1, self)?;
                }
                AbiType::Field => {
                    let witness = self.add_witness_to_cs();
                    struct_witnesses.insert(name.clone(), vec![witness]);
                }
                AbiType::Array { length, typ } => {
                    let internal_arr_witnesses = self.generate_array_witnesses(length, typ)?;
                    struct_witnesses.insert(name.clone(), internal_arr_witnesses);
                }
                AbiType::Struct { fields, .. } => {
                    let mut new_fields: BTreeMap<String, AbiType> = BTreeMap::new();
                    for (inner_name, value) in fields {
                        let new_name = format!("{name}.{inner_name}");
                        new_fields.insert(new_name, value.clone());
                    }
                    self.generate_struct_witnesses(struct_witnesses, &new_fields)?;
                }
                AbiType::String { length } => {
                    let typ = AbiType::Integer { sign: noirc_abi::Sign::Unsigned, width: 8 };
                    let internal_str_witnesses = self.generate_array_witnesses(length, &typ)?;
                    struct_witnesses.insert(name.clone(), internal_str_witnesses);
                }
            }
        }
        Ok(())
    }

    fn generate_array_witnesses(
        &mut self,
        length: &u64,
        typ: &AbiType,
    ) -> Result<Vec<Witness>, RuntimeErrorKind> {
        let mut witnesses = Vec::new();
        let element_width = match typ {
            AbiType::Integer { width, .. } => Some(*width),
            _ => None,
        };
        for _ in 0..*length {
            let witness = self.add_witness_to_cs();
            witnesses.push(witness);
            if let Some(ww) = element_width {
                ssa::acir_gen::range_constraint(witness, ww, self)?;
            }
        }
        Ok(witnesses)
    }

    /// The ABI is the intermediate representation between Noir and types like Toml
    /// Noted in the noirc_abi, it is possible to convert Toml -> NoirTypes
    /// However, this intermediate representation is useful as it allows us to have
    /// intermediate Types which the core type system does not know about like Strings.
    fn parse_abi_alt(&mut self, ir_gen: &mut IrGenerator) {
        // XXX: Currently, the syntax only supports public witnesses
        // u8 and arrays are assumed to be private
        // This is not a short-coming of the ABI, but of the grammar
        // The new grammar has been conceived, and will be implemented.
        let main = ir_gen.program.main_mut();
        let main_params = std::mem::take(&mut main.parameters);
        let abi_params = std::mem::take(&mut ir_gen.program.main_function_signature.0);

        assert_eq!(main_params.len(), abi_params.len());

        for ((param_id, _, param_name, _), abi_param) in main_params.iter().zip(abi_params) {
            assert_eq!(param_name, &abi_param.name);
            let def = Definition::Local(*param_id);
            self.param_to_var(param_name, def, &abi_param.typ, &abi_param.visibility, ir_gen)
                .unwrap();
        }

        // Store the number of witnesses used to represent the types
        // in the ABI
        self.num_witnesses_abi_len = self.current_witness_index as usize;
    }
}
