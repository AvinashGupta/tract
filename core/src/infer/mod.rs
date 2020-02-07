use std::fmt;
use downcast_rs::Downcast;
use crate::internal::*;

pub mod ops;
mod fact;
mod model;

pub use self::fact::InferenceFact;

/// A model with partially types and shapes, as produced by parsing ONNX or
/// Tensorflow graphs.
pub type InferenceModel = ModelImpl<InferenceFact, Box<dyn InferenceOp>>;
/// Node for InferenceModel graph
pub type InferenceNode = BaseNode<InferenceFact, Box<dyn InferenceOp>>;
/// A ModelPatch for InferenceModel.
pub type InferenceModelPatch = ModelPatch<InferenceFact, Box<dyn InferenceOp>>;
/// An execution plan for InferenceModel.
pub type InferenceSimplePlan<M> = SimplePlan<InferenceFact, Box<dyn InferenceOp>, M>;
/// An execution state for InferenceModel.
pub type InferenceSimpleState<M, P> = SimpleState<InferenceFact, Box<dyn InferenceOp>, M, P>;

/// An operation with tensor type inference
pub trait InferenceOp:
    Op + fmt::Debug + dyn_clone::DynClone + Send + Sync + 'static + Downcast + StatefullOp
{
    /// Infers properties about the input and output tensors.
    ///
    /// The `inputs` and `outputs` arguments correspond to properties about
    /// the input and output tensors that are already known.
    ///
    /// The default implementation will call the private infer_facts method,
    /// which is usually implemented using the InferenceRulesOp trait. It will
    /// also try to eval() the op if its a StatelessOp and if the inputs are
    /// fully determined.
    ///
    /// Returns Err in case of an unrecoverable error during the inference,
    /// and the refined properties about the inputs and outputs otherwise.
    fn infer(
        &mut self,
        inputs: TVec<&InferenceFact>,
        outputs: TVec<&InferenceFact>,
        observed: TVec<&InferenceFact>,
    ) -> TractResult<(TVec<InferenceFact>, TVec<InferenceFact>, TVec<InferenceFact>)> {
        let (infered_inputs, infered_outputs, observed) =
            self.infer_facts(inputs, outputs, observed).chain_err(|| "Infering facts")?;

        if let Some(stateless) = self.as_stateless() {
            if infered_inputs.iter().all(|i| i.value.is_concrete()) {
                let input_values = infered_inputs
                    .iter()
                    .map(|i| i.value.concretize().unwrap().clone().into())
                    .collect(); // checked
                match stateless.eval(input_values) {
                    Ok(values) => {
                        let output_values =
                            values.into_iter().map(|t| t.into()).collect::<TVec<_>>();
                        return Ok((infered_inputs, output_values, observed));
                    }
                    Err(e) => match e {
                        TractError(TractErrorKind::StreamTensor, _) => (),
                        e => return Err(e).chain_err(|| "Eager eval"),
                    },
                }
            }
        }

        return Ok((infered_inputs, infered_outputs, observed));
    }

    /// Allow an op to specify a supplementary list of outlets facts that
    /// will trigger inference again.
    fn observe_outlets(
        &self,
        _model: &InferenceModel,
        _node: &InferenceNode,
    ) -> TractResult<Vec<OutletId>> {
        Ok(vec![])
    }

    /// Infer properties about inputs and output tensors. This method does not
    /// need to deal with the "trivial" stateless op with fully determined
    /// inputs cases.
    ///
    /// Most of the time, it is implemented using InferenceRulesOp.
    fn infer_facts(
        &mut self,
        inputs: TVec<&InferenceFact>,
        outputs: TVec<&InferenceFact>,
        observed: TVec<&InferenceFact>,
    ) -> TractResult<(TVec<InferenceFact>, TVec<InferenceFact>, TVec<InferenceFact>)>;

    /// Early pass on inference model, after analyse, but before translation to
    /// typed network. Meant to deal with some framework idiosyncrasies that
    /// manifest with temporaries nodes that can run some form of inference but
    /// require refactoring the network before it can be evaluated.
    ///
    /// Called after succesful analyse, but before translating to typed model.
    #[allow(unused_variables)]
    fn incorporate(
        &self,
        model: &InferenceModel,
        node: &InferenceNode,
    ) -> TractResult<Option<InferenceModelPatch>> {
        Ok(None)
    }

    fn nboutputs(&self) -> TractResult<usize> {
        Ok(1)
    }

    /// Reinterpret the InferenceOp as an Op.
    fn as_op(&self) -> &dyn Op;

    /// Reinterpret the InferenceOp as an Op, mutably.
    fn as_op_mut(&mut self) -> &mut dyn Op;

    /// Called during translation to TypedModel.
    #[allow(unused_variables)]
    fn to_typed(
        &self,
        source: &InferenceModel,
        node: &InferenceNode,
        target: &mut TypedModel,
        mapping: &HashMap<OutletId, OutletId>,
    ) -> TractResult<TVec<OutletId>> {
        bail!("Operator can not be made a TypedOp.")
    }
}

impl std::fmt::Display for Box<dyn InferenceOp> {
    fn fmt(&self, fmt: &mut fmt::Formatter) -> fmt::Result {
        write!(fmt, "{}", self.name())
    }
}

impl<O: InferenceOp> From<O> for Box<dyn InferenceOp> {
    fn from(it: O) -> Box<dyn InferenceOp> {
        Box::new(it)
    }
}

impl AsRef<dyn Op> for dyn InferenceOp {
    fn as_ref(&self) -> &dyn Op {
        self.as_op()
    }
}

impl AsRef<dyn Op> for Box<dyn InferenceOp> {
    fn as_ref(&self) -> &dyn Op {
        self.as_op()
    }
}

impl AsMut<dyn Op> for dyn InferenceOp {
    fn as_mut(&mut self) -> &mut dyn Op {
        self.as_op_mut()
    }
}

impl AsMut<dyn Op> for Box<dyn InferenceOp> {
    fn as_mut(&mut self) -> &mut dyn Op {
        self.as_op_mut()
    }
}

