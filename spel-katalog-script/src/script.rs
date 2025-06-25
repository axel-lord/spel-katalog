//! Script header
use ::bon::Builder;
use ::serde::{Deserialize, Serialize};

use crate::exec::Exec;

/// A script specification.
#[derive(Debug, Clone, Deserialize, Serialize, Default, PartialEq, Eq, Builder)]
pub struct Script {
    /// Id of script.
    #[builder(start_fn, into)]
    pub id: String,

    /// Single executable run first.
    #[serde(default, skip_serializing_if = "Option::is_none", flatten)]
    #[builder(into)]
    pub exec: Option<Exec>,
}

impl Script {
    /// Visit all parsed string values.
    pub fn visit_strings<E>(
        &mut self,
        mut f: impl FnMut(&mut String) -> Result<(), E>,
    ) -> Result<(), E> {
        let Self { id, exec } = self;
        exec.iter_mut().try_for_each(|e| e.visit_strings(&mut f))?;
        f(id)
    }
}
