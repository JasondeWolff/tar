use std::{
    collections::{HashMap, HashSet},
    io::{Read, Write},
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};
use strum::EnumIter;
use uuid::Uuid;

use crate::render_graph::RenderGraph;

#[derive(Serialize, Deserialize, Clone, Copy, PartialEq, Eq, EnumIter)]
pub enum CodeFileType {
    Fragment,
    Compute,
    Shared,
}

impl CodeFileType {
    pub fn icon(&self) -> &'static str {
        match self {
            Self::Fragment => egui_phosphor::regular::IMAGE,
            Self::Compute => egui_phosphor::regular::CPU,
            Self::Shared => egui_phosphor::regular::CODE_SIMPLE,
        }
    }

    pub fn labeled_icon(&self) -> String {
        match self {
            Self::Fragment => format!("{} Fragment", self.icon()),
            Self::Compute => format!("{} Compute", self.icon()),
            Self::Shared => format!("{} Shared", self.icon()),
        }
    }

    pub fn file_extension(&self) -> &'static str {
        match self {
            Self::Fragment => "frag",
            Self::Compute => "comp",
            Self::Shared => "shared",
        }
    }
}

impl CodeFileType {
    pub fn default_source(&self) -> String {
        match self {
            Self::Fragment => include_str!("../assets/shaders/default.frag").to_owned(),
            Self::Compute => include_str!("../assets/shaders/default.comp").to_owned(),
            Self::Shared => include_str!("../assets/shaders/default.shared").to_owned(),
        }
    }
}

#[derive(Serialize, Deserialize)]
pub struct CodeFile {
    id: Uuid,
    relative_path: PathBuf,
    ty: CodeFileType,
    pub source: String,
}

impl CodeFile {
    pub fn new<P: Into<PathBuf>>(relative_path: P, ty: CodeFileType) -> Self {
        let source = ty.default_source();

        Self {
            id: Uuid::new_v4(),
            relative_path: relative_path.into(),
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

    pub fn relative_path(&self) -> &PathBuf {
        &self.relative_path
    }

    pub fn path(&self, code_path: &Path) -> PathBuf {
        code_path.join(&self.relative_path)
    }
}

#[derive(Serialize, Deserialize)]
pub struct CodeFiles {
    code_path: PathBuf,
    files: HashMap<Uuid, CodeFile>,
    /// Tracks explicitly created empty folders (not derived from file paths)
    #[serde(default)]
    extra_dirs: HashSet<PathBuf>,
}

impl CodeFiles {
    pub fn new<P: Into<PathBuf>>(project_path: P) -> Self {
        let project_path = project_path.into();
        let code_path = project_path.join("code");

        let mut code_files = Self {
            code_path,
            files: HashMap::new(),
            extra_dirs: HashSet::new(),
        };

        let _ = code_files.create_file("main", CodeFileType::Fragment);
        // let _ = code_files.create_file("bake_noise", CodeFileType::Compute);
        // let _ = code_files.create_file("atmosphere/march", CodeFileType::Fragment);
        // let _ = code_files.create_file("atmosphere/composite", CodeFileType::Fragment);
        // let _ = code_files.create_file("shared/common", CodeFileType::Shared);
        // let _ = code_files.create_file("shared/math", CodeFileType::Shared);
        // let _ = code_files.create_file("shared/bsdf/diffuse", CodeFileType::Shared);
        // let _ = code_files.create_file("shared/bsdf/dielectric", CodeFileType::Shared);
        // let _ = code_files.create_file("shared/bsdf/conductor", CodeFileType::Shared);
        // let _ = code_files.create_file("shared/bsdf/sampling", CodeFileType::Shared);

        // for i in 0..100 {
        //     let _ =
        //         code_files.create_file(format!("shared/bsdf/sampling_{}", i), CodeFileType::Shared);
        // }

        code_files
    }

    pub fn contains_file<P: AsRef<Path>>(&self, relative_path: P) -> bool {
        let relative_path = relative_path.as_ref();
        self.files
            .values()
            .any(|file| file.relative_path == relative_path)
    }

    pub fn create_file<P: Into<PathBuf>>(
        &mut self,
        relative_path: P,
        ty: CodeFileType,
    ) -> anyhow::Result<Uuid> {
        let relative_path = relative_path.into();
        let relative_path = relative_path.with_extension(ty.file_extension());

        if self.contains_file(&relative_path) {
            anyhow::bail!("Code file already exists");
        }

        // Remove any parent dirs from extra_dirs (they're now "real" folders)
        if let Some(parent) = relative_path.parent() {
            for ancestor in parent.ancestors() {
                if !ancestor.as_os_str().is_empty() {
                    self.extra_dirs.remove(ancestor);
                }
            }
        }

        let file = CodeFile::new(relative_path, ty);
        let id = file.id;
        self.files.insert(id, file);

        self.save_file(id)?;

        Ok(id)
    }

    /// Creates an empty folder and tracks it in extra_dirs
    pub fn create_folder<P: Into<PathBuf>>(&mut self, relative_path: P) -> anyhow::Result<()> {
        let relative_path = relative_path.into();

        // Check if this folder already exists (either as extra_dir or derived from files)
        if self.extra_dirs.contains(&relative_path) {
            anyhow::bail!("Folder already exists");
        }

        // Check if any file already implies this folder exists
        let folder_exists = self.files.values().any(|f| {
            f.relative_path
                .parent()
                .map(|p| p.starts_with(&relative_path) || p == relative_path)
                .unwrap_or(false)
        });

        if folder_exists {
            anyhow::bail!("Folder already exists");
        }

        // Create the directory on disk
        let full_path = self.code_path.join(&relative_path);
        std::fs::create_dir_all(&full_path)?;

        // Track it in extra_dirs
        self.extra_dirs.insert(relative_path);

        Ok(())
    }

    /// Checks if a folder exists (either as extra_dir or derived from files)
    pub fn contains_folder<P: AsRef<Path>>(&self, relative_path: P) -> bool {
        let relative_path = relative_path.as_ref();

        if self.extra_dirs.contains(relative_path) {
            return true;
        }

        // Check if any file implies this folder exists
        self.files.values().any(|f| {
            f.relative_path
                .parent()
                .map(|p| p.starts_with(relative_path) || p == relative_path)
                .unwrap_or(false)
        })
    }

    /// Returns iterator over extra_dirs
    pub fn extra_dirs_iter(&self) -> impl Iterator<Item = &PathBuf> {
        self.extra_dirs.iter()
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
            let path = code_file.path(&self.code_path);

            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            let mut file = std::fs::File::create(path)?;
            file.write_all(code_file.source.as_bytes())?;

            Ok(())
        } else {
            anyhow::bail!("No code file found with id {}", id);
        }
    }

    pub fn save_all(&self) -> anyhow::Result<()> {
        for id in self.files.keys() {
            self.save_file(*id)?;
        }

        Ok(())
    }

    pub fn load_file(&mut self, id: Uuid) -> anyhow::Result<()> {
        if let Some(code_file) = self.files.get_mut(&id) {
            let mut file = std::fs::File::open(code_file.path(&self.code_path))?;

            let mut loaded_src = String::new();
            file.read_to_string(&mut loaded_src)?;
            code_file.source = loaded_src;

            Ok(())
        } else {
            anyhow::bail!("No code file found with id {}", id);
        }
    }

    pub fn load_all(&mut self) -> anyhow::Result<()> {
        for id in self.files.keys().copied().collect::<Vec<Uuid>>() {
            self.load_file(id)?;
        }

        // Scan for empty directories on disk and add them to extra_dirs
        self.scan_empty_dirs()?;

        Ok(())
    }

    /// Scans the code directory for empty folders and adds them to extra_dirs
    fn scan_empty_dirs(&mut self) -> anyhow::Result<()> {
        self.scan_empty_dirs_recursive(&self.code_path.clone(), &PathBuf::new())
    }

    fn scan_empty_dirs_recursive(
        &mut self,
        full_path: &Path,
        relative_path: &Path,
    ) -> anyhow::Result<()> {
        if !full_path.is_dir() {
            return Ok(());
        }

        let entries: Vec<_> = std::fs::read_dir(full_path)?
            .filter_map(|e| e.ok())
            .collect();

        let mut has_files = false;
        let mut subdirs = Vec::new();

        for entry in entries {
            let path = entry.path();
            if path.is_dir() {
                subdirs.push(path);
            } else {
                has_files = true;
            }
        }

        // Recursively scan subdirectories
        for subdir in &subdirs {
            let subdir_name = subdir.file_name().unwrap();
            let subdir_relative = relative_path.join(subdir_name);
            self.scan_empty_dirs_recursive(subdir, &subdir_relative)?;
        }

        // If this directory has no files and no subdirs, it's empty
        // Also check if this folder is not already implied by existing files
        if !has_files && subdirs.is_empty() && !relative_path.as_os_str().is_empty() {
            let folder_implied_by_files = self.files.values().any(|f| {
                f.relative_path
                    .parent()
                    .map(|p| p == relative_path || p.starts_with(relative_path))
                    .unwrap_or(false)
            });

            if !folder_implied_by_files {
                self.extra_dirs.insert(relative_path.to_path_buf());
            }
        }

        Ok(())
    }

    pub fn move_file<P: Into<PathBuf>>(
        &mut self,
        id: Uuid,
        new_relative_path: P,
    ) -> anyhow::Result<()> {
        let new_relative_path = new_relative_path.into();

        if self.contains_file(&new_relative_path) {
            anyhow::bail!("New path '{:?}' already exists", &new_relative_path);
        }

        if let Some(code_file) = self.files.get_mut(&id) {
            if code_file.relative_path != new_relative_path {
                let old_path = code_file.path(&self.code_path);

                code_file.relative_path = new_relative_path;
                let new_path = code_file.path(&self.code_path);

                if let Some(parent) = new_path.parent() {
                    std::fs::create_dir_all(parent)?;
                }

                std::fs::rename(old_path, new_path)?;
            }

            Ok(())
        } else {
            anyhow::bail!("No code file found with id {}", id);
        }
    }

    pub fn move_folder<P: Into<PathBuf>>(
        &mut self,
        old_relative_path: PathBuf,
        new_relative_path: P,
    ) -> anyhow::Result<()> {
        let new_relative_path: PathBuf = new_relative_path.into();

        if old_relative_path != new_relative_path {
            let old_path = self.code_path.join(&old_relative_path);
            let new_path = self.code_path.join(&new_relative_path);

            if let Some(parent) = new_path.parent() {
                std::fs::create_dir_all(parent)?;
            }

            std::fs::rename(old_path, new_path)?;

            // Update file paths
            for file in self.files.values_mut() {
                if file.relative_path.starts_with(&old_relative_path) {
                    if let Ok(suffix) = file.relative_path.strip_prefix(&old_relative_path) {
                        file.relative_path = new_relative_path.join(suffix);
                    }
                }
            }

            // Update extra_dirs paths
            let dirs_to_update: Vec<PathBuf> = self
                .extra_dirs
                .iter()
                .filter(|p| p.starts_with(&old_relative_path) || **p == old_relative_path)
                .cloned()
                .collect();

            for old_dir in dirs_to_update {
                self.extra_dirs.remove(&old_dir);
                if old_dir == old_relative_path {
                    self.extra_dirs.insert(new_relative_path.clone());
                } else if let Ok(suffix) = old_dir.strip_prefix(&old_relative_path) {
                    self.extra_dirs.insert(new_relative_path.join(suffix));
                }
            }
        }

        Ok(())
    }

    pub fn delete_file(&mut self, id: Uuid) -> anyhow::Result<()> {
        if let Some(file) = self.files.remove(&id) {
            std::fs::remove_file(file.path(&self.code_path))?;
        } else {
            anyhow::bail!("No code file found with id {}", id);
        }

        Ok(())
    }

    /// Deletes a folder and all files within it
    pub fn delete_folder<P: AsRef<Path>>(&mut self, relative_path: P) -> anyhow::Result<()> {
        let relative_path = relative_path.as_ref();

        // Remove from extra_dirs if it's there
        self.extra_dirs.remove(relative_path);

        // Also remove any extra_dirs that are children of this folder
        let child_extra_dirs: Vec<PathBuf> = self
            .extra_dirs
            .iter()
            .filter(|p| p.starts_with(relative_path))
            .cloned()
            .collect();
        for child in child_extra_dirs {
            self.extra_dirs.remove(&child);
        }

        // Remove all files within this folder
        let files_to_delete: Vec<Uuid> = self
            .files
            .iter()
            .filter(|(_, f)| f.relative_path.starts_with(relative_path))
            .map(|(id, _)| *id)
            .collect();

        for id in files_to_delete {
            self.files.remove(&id);
        }

        // Delete the folder from disk
        let full_path = self.code_path.join(relative_path);
        if full_path.exists() {
            std::fs::remove_dir_all(full_path)?;
        }

        Ok(())
    }

    pub fn files_iter(&self) -> impl Iterator<Item = (&Uuid, &CodeFile)> {
        self.files.iter()
    }

    pub fn get_file(&self, id: Uuid) -> Option<&CodeFile> {
        self.files.get(&id)
    }
}

// TODO: after deserialize (so load), also override the CodeFiles sources with what's on disk, if it can be found
// so basically a soft load, don't care if it fails to load any files, we still got the source from the project
// after this step, also perform a save, to again make sure the missing files on disk (but still present in the project) are pooped out to disk
#[derive(Serialize, Deserialize)]
pub struct Project {
    path: PathBuf,
    render_graph: RenderGraph,
    pub code_files: CodeFiles,
}

impl Project {
    pub fn new<P: Into<PathBuf>>(path: P) -> Self {
        let path = path.into();
        let code_files = CodeFiles::new(path.parent().unwrap());
        let render_graph = RenderGraph::new(&code_files);

        Self {
            path,
            code_files,
            render_graph,
        }
    }

    pub fn render_graph(&self) -> &RenderGraph {
        &self.render_graph
    }

    pub fn render_graph_mut(&mut self) -> &mut RenderGraph {
        &mut self.render_graph
    }

    pub fn save(&self) -> anyhow::Result<()> {
        let file = std::fs::File::create(&self.path)?;
        let writer = std::io::BufWriter::new(file);
        serde_json::to_writer(writer, &self)?;

        self.code_files.save_all()?;

        Ok(())
    }

    pub fn load(path: &Path) -> anyhow::Result<Self> {
        let file = std::fs::File::open(path)?;
        let reader = std::io::BufReader::new(file);
        let mut data: Project = serde_json::from_reader(reader)?;

        data.code_files.load_all()?;

        Ok(data)
    }

    pub fn get_file_icon(&self, path: &Path, id: Uuid) -> &'static str {
        if let Some(file) = self.code_files.get_file(id) {
            return file.ty().icon();
        }

        match path.extension().and_then(|s| s.to_str()) {
            Some("wgsl") => egui_phosphor::regular::FILE_CODE,
            Some("glsl") => egui_phosphor::regular::FILE_CODE,
            Some("json") => egui_phosphor::regular::BRACKETS_CURLY,
            Some("toml") | Some("yaml") | Some("yml") => egui_phosphor::regular::GEAR,
            _ => egui_phosphor::regular::FILE,
        }
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
