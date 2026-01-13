use std::{
    collections::HashMap,
    io::{Read, Write},
    path::{Path, PathBuf},
};

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
pub struct CodeFiles {
    path: PathBuf,
    files: HashMap<Uuid, CodeFile>,
}

impl CodeFiles {
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        let mut code_files = Self {
            path: path.into(),
            files: HashMap::new(),
        };
        let _ = code_files.create_file("main.frag.wgsl", CodeFileType::Fragment);
        code_files
    }

    pub fn contains_file<P: AsRef<Path>>(&self, path: P) -> bool {
        let path = path.as_ref();
        self.files.values().any(|file| file.path == path)
    }

    pub fn create_file<P: Into<PathBuf>>(
        &mut self,
        path: P,
        ty: CodeFileType,
    ) -> anyhow::Result<Uuid> {
        let path = path.into();
        if self.contains_file(&path) {
            anyhow::bail!("Code file already exists");
        }

        let file = CodeFile::new(path, ty);
        let id = file.id;
        self.files.insert(id, file);

        self.save_file(id)?;

        Ok(id)
    }

    pub fn get_source(&self, id: Uuid) -> anyhow::Result<String> {
        if let Some(code_file) = self.files.get(&id) {
            Ok(code_file.source.clone())
        } else {
            anyhow::bail!("No code file found with id {}", id);
        }
    }

    pub fn set_source<S: Into<String>>(&mut self, id: Uuid, source: S) -> anyhow::Result<()> {
        if let Some(code_file) = self.files.get_mut(&id) {
            code_file.source = source.into();
            Ok(())
        } else {
            anyhow::bail!("No code file found with id {}", id);
        }
    }

    pub fn save_file(&self, id: Uuid) -> anyhow::Result<()> {
        if let Some(code_file) = self.files.get(&id) {
            let mut file = std::fs::File::create(&code_file.path)?;
            file.write_all(code_file.source.as_bytes())?;

            Ok(())
        } else {
            anyhow::bail!("No code file found with id {}", id);
        }
    }

    pub fn load_file(&mut self, id: Uuid) -> anyhow::Result<()> {
        if let Some(code_file) = self.files.get_mut(&id) {
            let mut file = std::fs::File::open(&code_file.path)?;
            file.read_to_string(&mut code_file.source)?;

            Ok(())
        } else {
            anyhow::bail!("No code file found with id {}", id);
        }
    }

    pub fn move_file<P: Into<PathBuf>>(&mut self, id: Uuid, new_path: P) -> anyhow::Result<()> {
        let new_path = new_path.into();

        if self.contains_file(&new_path) {
            anyhow::bail!("New path '{:?}' already exists", &new_path);
        }

        if let Some(code_file) = self.files.get_mut(&id) {
            if code_file.path != new_path {
                std::fs::rename(&code_file.path, &new_path)?;
                code_file.path = new_path;
            }

            Ok(())
        } else {
            anyhow::bail!("No code file found with id {}", id);
        }
    }

    pub fn delete_file(&mut self, id: Uuid) -> anyhow::Result<()> {
        if let Some(file) = self.files.remove(&id) {
            std::fs::remove_file(&file.path)?;
        } else {
            anyhow::bail!("No code file found with id {}", id);
        }

        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
pub struct Project {
    path: PathBuf,
    code_files: CodeFiles,
    render_graph: RenderGraph,
}

impl Project {
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        let path = path.into();
        let code_files = CodeFiles::new(path.clone());
        let render_graph = RenderGraph::new();

        Self {
            path,
            code_files,
            render_graph,
        }
    }
}
