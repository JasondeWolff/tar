use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use wgpu::naga::{
    front::wgsl,
    valid::{Capabilities, ValidationFlags, Validator},
};

#[derive(Serialize, Deserialize)]
pub struct RenderGraph {}

impl RenderGraph {
    pub fn new() -> Self {
        Self {}
    }

    pub fn compile(&mut self) -> Result<(), String> {
        // let mut frontend = wgsl::Frontend::new();

        // let module = frontend
        //     .parse(&self.source)
        //     .map_err(|e| e.emit_to_string(&self.source))?;

        // let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());

        // validator.validate(&module).map_err(|e| format!("{e:?}"))?;

        Ok(())
    }
}
