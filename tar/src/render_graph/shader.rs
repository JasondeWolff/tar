use wgpu::naga::{
    front::wgsl,
    valid::{Capabilities, ValidationFlags, Validator},
    StorageAccess,
};

use crate::render_graph::RgDataType;

#[derive(Debug, Clone)]
pub enum ShaderBindingLayout {
    SampledTexture {
        dim: wgpu::TextureViewDimension,
        sample_type: wgpu::TextureSampleType,
        multisampled: bool,
    },
    StorageTexture {
        dim: wgpu::TextureViewDimension,
        format: wgpu::TextureFormat,
        access: wgpu::StorageTextureAccess,
    },
    UniformBuffer,
    StorageBuffer {
        read_only: bool,
    },
    Sampler(wgpu::SamplerBindingType),
}

#[derive(Debug, Clone)]
pub struct ShaderBinding {
    pub set: u32,
    pub binding: u32,
    pub name: String,
    pub resource_type: RgDataType,
    pub readonly: bool,
    pub layout_type: ShaderBindingLayout,
}

impl ShaderBinding {
    pub fn to_layout_entry(&self) -> wgpu::BindGroupLayoutEntry {
        let ty = match &self.layout_type {
            ShaderBindingLayout::SampledTexture {
                dim,
                sample_type,
                multisampled,
            } => wgpu::BindingType::Texture {
                sample_type: *sample_type,
                view_dimension: *dim,
                multisampled: *multisampled,
            },
            ShaderBindingLayout::StorageTexture {
                dim,
                format,
                access,
            } => wgpu::BindingType::StorageTexture {
                access: *access,
                format: *format,
                view_dimension: *dim,
            },
            ShaderBindingLayout::UniformBuffer => wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Uniform,
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            ShaderBindingLayout::StorageBuffer { read_only } => wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage {
                    read_only: *read_only,
                },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            ShaderBindingLayout::Sampler(t) => wgpu::BindingType::Sampler(*t),
        };

        wgpu::BindGroupLayoutEntry {
            binding: self.binding,
            visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
            ty,
            count: None,
        }
    }
}

pub struct Shader {
    src: String,
    shader_module: Option<wgpu::ShaderModule>,
    bindings: Vec<ShaderBinding>,
    errors: Vec<(String, Option<u32>)>,
    warnings: Vec<(String, Option<u32>)>,
}

impl Shader {
    pub fn new(src: String, device: &wgpu::Device) -> Self {
        let mut shader = Self {
            src,
            shader_module: None,
            bindings: Vec::new(),
            errors: Vec::new(),
            warnings: Vec::new(),
        };

        shader.compile(device);
        shader
    }

    fn compile(&mut self, device: &wgpu::Device) {
        self.errors.clear();
        self.warnings.clear();

        // Parse the WGSL
        let module = match wgsl::parse_str(&self.src) {
            Ok(module) => module,
            Err(parse_error) => {
                let line = parse_error
                    .labels()
                    .next()
                    .map(|(span, _)| span.location(&self.src).line_number);

                self.errors.push((format!("{}", parse_error), line));
                return;
            }
        };

        // Validate the module
        let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
        let _module_info = match validator.validate(&module) {
            Ok(info) => info,
            Err(validation_error) => {
                let line = validation_error
                    .spans()
                    .next()
                    .map(|(span, _)| span.location(&self.src).line_number);

                self.errors.push((format!("{}", validation_error), line));
                return;
            }
        };

        let mut new_bindings = Vec::new();

        // Extract bindings from the validated module
        for (_handle, global) in module.global_variables.iter() {
            if let Some(binding) = &global.binding {
                let mut readonly = true;

                let (resource_type, layout_type) = match module.types[global.ty].inner {
                    wgpu::naga::TypeInner::Image {
                        dim,
                        arrayed,
                        class,
                    } => {
                        let view_dim = naga_image_dim_to_wgpu(dim, arrayed);
                        let rg_type = if !arrayed {
                            match dim {
                                wgpu::naga::ImageDimension::D1 => RgDataType::UInt,
                                wgpu::naga::ImageDimension::D2 => RgDataType::Tex2D,
                                wgpu::naga::ImageDimension::D3 => RgDataType::Tex3D,
                                wgpu::naga::ImageDimension::Cube => RgDataType::UInt,
                            }
                        } else {
                            match dim {
                                wgpu::naga::ImageDimension::D1 => RgDataType::UInt,
                                wgpu::naga::ImageDimension::D2 => RgDataType::Tex2DArray,
                                wgpu::naga::ImageDimension::D3 => RgDataType::UInt,
                                wgpu::naga::ImageDimension::Cube => RgDataType::UInt,
                            }
                        };
                        let layout = match class {
                            wgpu::naga::ImageClass::Sampled { kind, multi } => {
                                ShaderBindingLayout::SampledTexture {
                                    dim: view_dim,
                                    sample_type: naga_scalar_kind_to_sample_type(kind),
                                    multisampled: multi,
                                }
                            }
                            wgpu::naga::ImageClass::Depth { multi } => {
                                ShaderBindingLayout::SampledTexture {
                                    dim: view_dim,
                                    sample_type: wgpu::TextureSampleType::Depth,
                                    multisampled: multi,
                                }
                            }
                            wgpu::naga::ImageClass::Storage { format, access } => {
                                if access == StorageAccess::STORE {
                                    readonly = false;
                                }
                                ShaderBindingLayout::StorageTexture {
                                    dim: view_dim,
                                    format: naga_storage_format_to_wgpu(format),
                                    access: naga_storage_access_to_wgpu(access),
                                }
                            }
                        };
                        (rg_type, layout)
                    }
                    wgpu::naga::TypeInner::Sampler { comparison } => {
                        let binding_type = if comparison {
                            wgpu::SamplerBindingType::Comparison
                        } else {
                            wgpu::SamplerBindingType::Filtering
                        };
                        (RgDataType::UInt, ShaderBindingLayout::Sampler(binding_type))
                    }
                    wgpu::naga::TypeInner::Struct { .. } | wgpu::naga::TypeInner::Array { .. } => {
                        match global.space {
                            wgpu::naga::AddressSpace::Uniform => {
                                (RgDataType::Buffer, ShaderBindingLayout::UniformBuffer)
                            }
                            wgpu::naga::AddressSpace::Storage { access } => {
                                let read_only = access != StorageAccess::STORE;
                                if !read_only {
                                    readonly = false;
                                }
                                (
                                    RgDataType::Buffer,
                                    ShaderBindingLayout::StorageBuffer { read_only },
                                )
                            }
                            _ => (RgDataType::UInt, ShaderBindingLayout::UniformBuffer),
                        }
                    }
                    _ => (RgDataType::UInt, ShaderBindingLayout::UniformBuffer),
                };

                new_bindings.push(ShaderBinding {
                    set: binding.group,
                    binding: binding.binding,
                    name: global
                        .name
                        .clone()
                        .unwrap_or_else(|| "unnamed_binding".to_string()),
                    resource_type,
                    readonly,
                    layout_type,
                });
            }
        }

        new_bindings.sort_by_key(|b| (b.set, b.binding));

        if self.errors.is_empty() {
            let shader_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some("Shader Module"),
                source: wgpu::ShaderSource::Wgsl(self.src.as_str().into()),
            });

            self.shader_module = Some(shader_module);
            self.bindings = new_bindings;
        }
    }

    pub fn shader_module(&self) -> &Option<wgpu::ShaderModule> {
        &self.shader_module
    }

    pub fn get_bindings(&self) -> &[ShaderBinding] {
        &self.bindings
    }

    pub fn get_errors(&self) -> &[(String, Option<u32>)] {
        &self.errors
    }

    pub fn get_warnings(&self) -> &[(String, Option<u32>)] {
        &self.warnings
    }

    pub fn get_source(&self) -> &str {
        &self.src
    }

    pub fn update_source(&mut self, new_src: String, device: &wgpu::Device) -> bool {
        if new_src != self.src {
            self.src = new_src;
            self.compile(device);
            true
        } else {
            false
        }
    }
}

fn naga_image_dim_to_wgpu(
    dim: wgpu::naga::ImageDimension,
    arrayed: bool,
) -> wgpu::TextureViewDimension {
    match (dim, arrayed) {
        (wgpu::naga::ImageDimension::D1, false) => wgpu::TextureViewDimension::D1,
        (wgpu::naga::ImageDimension::D2, false) => wgpu::TextureViewDimension::D2,
        (wgpu::naga::ImageDimension::D2, true) => wgpu::TextureViewDimension::D2Array,
        (wgpu::naga::ImageDimension::D3, false) => wgpu::TextureViewDimension::D3,
        (wgpu::naga::ImageDimension::Cube, false) => wgpu::TextureViewDimension::Cube,
        (wgpu::naga::ImageDimension::Cube, true) => wgpu::TextureViewDimension::CubeArray,
        _ => wgpu::TextureViewDimension::D2,
    }
}

fn naga_scalar_kind_to_sample_type(kind: wgpu::naga::ScalarKind) -> wgpu::TextureSampleType {
    match kind {
        wgpu::naga::ScalarKind::Float => wgpu::TextureSampleType::Float { filterable: true },
        wgpu::naga::ScalarKind::Sint => wgpu::TextureSampleType::Sint,
        wgpu::naga::ScalarKind::Uint => wgpu::TextureSampleType::Uint,
        _ => wgpu::TextureSampleType::Float { filterable: true },
    }
}

fn naga_storage_access_to_wgpu(access: wgpu::naga::StorageAccess) -> wgpu::StorageTextureAccess {
    match (
        access.contains(wgpu::naga::StorageAccess::LOAD),
        access.contains(wgpu::naga::StorageAccess::STORE),
    ) {
        (true, true) => wgpu::StorageTextureAccess::ReadWrite,
        (false, true) => wgpu::StorageTextureAccess::WriteOnly,
        _ => wgpu::StorageTextureAccess::ReadOnly,
    }
}

fn naga_storage_format_to_wgpu(format: wgpu::naga::StorageFormat) -> wgpu::TextureFormat {
    use wgpu::naga::StorageFormat as SF;
    match format {
        SF::R8Unorm => wgpu::TextureFormat::R8Unorm,
        SF::R8Snorm => wgpu::TextureFormat::R8Snorm,
        SF::R8Uint => wgpu::TextureFormat::R8Uint,
        SF::R8Sint => wgpu::TextureFormat::R8Sint,
        SF::R16Uint => wgpu::TextureFormat::R16Uint,
        SF::R16Sint => wgpu::TextureFormat::R16Sint,
        SF::R16Unorm => wgpu::TextureFormat::R16Unorm,
        SF::R16Snorm => wgpu::TextureFormat::R16Snorm,
        SF::R16Float => wgpu::TextureFormat::R16Float,
        SF::Rg8Unorm => wgpu::TextureFormat::Rg8Unorm,
        SF::Rg8Snorm => wgpu::TextureFormat::Rg8Snorm,
        SF::Rg8Uint => wgpu::TextureFormat::Rg8Uint,
        SF::Rg8Sint => wgpu::TextureFormat::Rg8Sint,
        SF::R32Uint => wgpu::TextureFormat::R32Uint,
        SF::R32Sint => wgpu::TextureFormat::R32Sint,
        SF::R32Float => wgpu::TextureFormat::R32Float,
        SF::Rg16Uint => wgpu::TextureFormat::Rg16Uint,
        SF::Rg16Sint => wgpu::TextureFormat::Rg16Sint,
        SF::Rg16Unorm => wgpu::TextureFormat::Rg16Unorm,
        SF::Rg16Snorm => wgpu::TextureFormat::Rg16Snorm,
        SF::Rg16Float => wgpu::TextureFormat::Rg16Float,
        SF::Rgba8Unorm => wgpu::TextureFormat::Rgba8Unorm,
        SF::Rgba8Snorm => wgpu::TextureFormat::Rgba8Snorm,
        SF::Rgba8Uint => wgpu::TextureFormat::Rgba8Uint,
        SF::Rgba8Sint => wgpu::TextureFormat::Rgba8Sint,
        SF::Rgb10a2Uint => wgpu::TextureFormat::Rgb10a2Uint,
        SF::Rgb10a2Unorm => wgpu::TextureFormat::Rgb10a2Unorm,
        SF::Rg11b10Ufloat => wgpu::TextureFormat::Rg11b10Ufloat,
        SF::Rg32Uint => wgpu::TextureFormat::Rg32Uint,
        SF::Rg32Sint => wgpu::TextureFormat::Rg32Sint,
        SF::Rg32Float => wgpu::TextureFormat::Rg32Float,
        SF::Rgba16Uint => wgpu::TextureFormat::Rgba16Uint,
        SF::Rgba16Sint => wgpu::TextureFormat::Rgba16Sint,
        SF::Rgba16Unorm => wgpu::TextureFormat::Rgba16Unorm,
        SF::Rgba16Snorm => wgpu::TextureFormat::Rgba16Snorm,
        SF::Rgba16Float => wgpu::TextureFormat::Rgba16Float,
        SF::Rgba32Uint => wgpu::TextureFormat::Rgba32Uint,
        SF::Rgba32Sint => wgpu::TextureFormat::Rgba32Sint,
        SF::Rgba32Float => wgpu::TextureFormat::Rgba32Float,
        SF::Bgra8Unorm => wgpu::TextureFormat::Bgra8Unorm,
        SF::R64Uint => wgpu::TextureFormat::R64Uint,
    }
}
