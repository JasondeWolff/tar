use std::{collections::HashMap, num::NonZeroU32, path::PathBuf};

use serde::{Deserialize, Serialize};
use strum::{EnumIter, EnumString};
use uuid::Uuid;
use wgpu::naga::{
    front::wgsl,
    valid::{Capabilities, ValidationFlags, Validator},
};

use crate::wgpu_util::BasicColorTextureFormat;

#[derive(
    Default,
    Copy,
    Clone,
    Debug,
    PartialEq,
    serde::Serialize,
    serde::Deserialize,
    strum::EnumIter,
    strum::Display,
)]
pub enum ScreenTexResolution {
    #[default]
    Full,
    Half,
    Quarter,
}

#[derive(Default, Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct ScreenTex {
    pub resolution: ScreenTexResolution,
    pub mipmaps: u32,
    pub format: BasicColorTextureFormat,
    pub persistent: bool,
}

#[derive(Default, Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct HistoryScreenTex {
    pub resolution: ScreenTexResolution,
    pub mipmaps: u32,
    pub format: BasicColorTextureFormat,
}

#[derive(Default, Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Tex2D {
    pub resolution: [u32; 2],
    pub mipmaps: u32,
    pub format: BasicColorTextureFormat,
    pub persistent: bool,
}

#[derive(Default, Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct HistoryTex2D {
    pub resolution: [u32; 2],
    pub mipmaps: u32,
    pub format: BasicColorTextureFormat,
}

#[derive(Default, Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Tex2DArray {
    pub resolution: [u32; 2],
    pub count: u32,
    pub mipmaps: u32,
    pub format: BasicColorTextureFormat,
    pub persistent: bool,
}

#[derive(Default, Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Tex3D {
    pub resolution: [u32; 3],
    pub mipmaps: u32,
    pub format: BasicColorTextureFormat,
    pub persistent: bool,
}

#[derive(Default, Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct HistoryTex3D {
    pub resolution: [u32; 3],
    pub mipmaps: u32,
    pub format: BasicColorTextureFormat,
}

#[derive(Default, Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub struct Tex3DArray {
    pub resolution: [u32; 3],
    pub count: u32,
    pub mipmaps: u32,
    pub format: BasicColorTextureFormat,
    pub persistent: bool,
}

#[derive(PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum RgDataType {
    UInt,
    UInt2,
    UInt3,
    Float,
    Bool,

    ScreenTexResolution,
    TextureFormat,

    Tex2D,
    HistoryTex2D,
    Tex2DArray,
    Tex3D,
    HistoryTex3D,
    Tex3DArray,
}

#[derive(Copy, Clone, Debug, serde::Serialize, serde::Deserialize)]
pub enum RgValueType {
    UInt(u32),
    UInt2([u32; 2]),
    UInt3([u32; 3]),
    Float(f32),
    Bool(bool),

    ScreenTexResolution(ScreenTexResolution),
    TextureFormat(BasicColorTextureFormat),

    ScreenTex(ScreenTex),
    Tex2D(Tex2D),
    Tex2DArray(Tex2DArray),
    Tex3D(Tex3D),
    Tex3DArray(Tex3DArray),
}

impl Default for RgValueType {
    fn default() -> Self {
        Self::UInt(0)
    }
}

#[derive(Clone, Copy, serde::Serialize, serde::Deserialize, EnumIter)]
pub enum RgNodeTemplate {
    ScreenTex,
    HistoryScreenTex,
    Tex2D,
    HistoryTex2D,
    Tex2DArray,
    Tex3D,
    HistoryTex3D,
    Tex3DArray,

    GraphicsPass,

    DisplayOut,
}

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
