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

// TODO: after deserialize (so load), also override the CodeFiles sources with what's on disk, if it can be found
// so basically a soft load, don't care if it fails to load any files, we still got the source from the project
// after this step, also perform a save, to again make sure the missing files on disk (but still present in the project) are pooped out to disk
#[derive(Serialize, Deserialize)]
pub struct Project {
    path: PathBuf,
    render_graph: RenderGraph,
    code_files: CodeFiles,
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

    pub fn render_graph_mut(&mut self) -> &mut RenderGraph {
        &mut self.render_graph
    }
}

#[cfg(target_os = "android")]
pub fn default_project_path() -> Option<PathBuf> {
    get_external_files_dir()
}

#[cfg(target_os = "android")]
fn get_external_files_dir() -> Option<PathBuf> {
    use jni::objects::{JObject, JValue};
    use ndk_context::android_context;

    let ctx = android_context();

    // Get JavaVM from the global context
    let vm = unsafe { jni::JavaVM::from_raw(ctx.vm().cast()).ok()? };
    let mut env = vm.attach_current_thread().ok()?;

    // Get the activity/context object
    let context = unsafe { JObject::from_raw(ctx.context().cast()) };

    // Call getExternalFilesDir(null)
    let null_obj = JObject::null();
    let files_dir = env
        .call_method(
            &context,
            "getExternalFilesDir",
            "(Ljava/lang/String;)Ljava/io/File;",
            &[JValue::Object(&null_obj)],
        )
        .ok()?
        .l()
        .ok()?;

    if files_dir.is_null() {
        return None;
    }

    // Call getAbsolutePath() on the File object
    let path_jstring = env
        .call_method(&files_dir, "getAbsolutePath", "()Ljava/lang/String;", &[])
        .ok()?
        .l()
        .ok()?;

    let path_str = env.get_string((&path_jstring).into()).ok()?;

    Some(PathBuf::from(path_str.into()))
}

#[cfg(not(target_os = "android"))]
pub fn default_project_path() -> Option<PathBuf> {
    directories::UserDirs::new()
        .and_then(|user_dirs| user_dirs.document_dir().map(|dir| dir.to_path_buf()))
}
