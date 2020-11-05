use crate::{Environment, Evaluator, Linear, Object, EvaluatorError};


// Intentionally chose to write this out manually as it's not expected to change often or at all
// We could expand again, so that ordering is preserved, but this does not seem necessary.
pub fn handle_add_op(
    left: Object,
    right: Object,
    env: &mut Environment,
    evaluator: &mut Evaluator,
) -> Result<Object, EvaluatorError> {
    match (left, right) {
        //
        // You cannot add Null objects with anything else
        (Object::Null, _) | (_, Object::Null) => Err(handle_cannot_add("()")),
        //
        // You cannot add array objects with anything else, currently
        (Object::Array(_), _) | (_, Object::Array(_)) => Err(handle_cannot_add("Arrays")),
        //
        // Delegate logic for integer addition to the integer module
        (Object::Integer(x), y) | (y,Object::Integer(x))=> Ok(Object::Integer(x.add(y, env, evaluator)?)),
        //
        // Arith + Arith = Arith
        (Object::Arithmetic(x), Object::Arithmetic(y)) => Ok(Object::Arithmetic(&x + &y)),
        //
        // Arith + Linear = Linear + Arith = Arith
        (Object::Linear(x), Object::Arithmetic(y)) | (Object::Arithmetic(y), Object::Linear(x)) => Ok(Object::Arithmetic(&x + &y)),
        //
        // Arith + Constant = Arith + Linear
        (Object::Constants(x), Object::Arithmetic(y)) | (Object::Arithmetic(y), Object::Constants(x)) => Ok(Object::Arithmetic(&y + &Linear::from(x))),
        //
        // Linear + Constant = Constant + Linear = Linear
        (Object::Constants(x), Object::Linear(y)) | (Object::Linear(y), Object::Constants(x)) => Ok(Object::Linear(&y + &x)),
        //
        // Linear + Linear = Arithmetic
        (Object::Linear(x), Object::Linear(y)) => Ok(Object::Arithmetic(x + y)),
        //
        // Constant + Constant = Constant
        (Object::Constants(x), Object::Constants(y)) => Ok(Object::Constants(x + y)),
    }
}

fn handle_cannot_add(typ : &'static str) -> EvaluatorError {
    EvaluatorError::UnstructuredError{span : Default::default(), message : format!("{} cannot be used in an addition", typ)}
}