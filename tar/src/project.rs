use std::{collections::HashMap, path::PathBuf};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::render_graph::RenderGraph;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq)]
pub enum CodeFileType {
    Fragment,
    Compute,
    Shared,
}

impl CodeFileType {
    pub fn default_source(&self) -> String {
        match self {
            Self::Fragment => String::from(
                r#"
@fragment
fn main(tex_coord: vec2f) -> @location(0) vec4f {
    return vec4f(tex_coord, 0.0, 1.0);
}
"#,
            ),
            Self::Compute => String::from(
                r#"
@compute
@workgroup_size(16, 16)
fn main(@builtin(global_invocation_id) global_id: vec3<u32>) {
    
}
"#,
            ),
            Self::Shared => String::from(
                r#"
fn my_func(a: u32, b: u32) -> u32 {
    return a + b;
}
"#,
            ),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct CodeFile {
    id: Uuid,
    path: PathBuf,
    ty: CodeFileType,
    source: String,
}

impl CodeFile {
    pub fn new<P: Into<PathBuf>>(path: P, ty: CodeFileType) -> Self {
        let source = ty.default_source();

        Self {
            id: Uuid::new_v4(),
            path: path.into(),
            ty,
            source,
        }
    }

    pub fn id(&self) -> Uuid {
        self.id
    }

    pub fn ty(&self) -> CodeFileType {
        self.ty
    }
}

#[derive(Serialize, Deserialize)]
pub struct Project {
    pub name: String,
    code_files: HashMap<Uuid, CodeFile>,
    render_graph: RenderGraph,
}

impl Project {
    pub fn new(name: String) -> Self {
        let main_file = CodeFile::new("main.frag.wgsl", CodeFileType::Fragment);
        let mut code_files = HashMap::new();
        code_files.insert(main_file.id, main_file);

        let render_graph = RenderGraph::new();

        Self {
            name,
            code_files,
            render_graph,
        }
    }
}
