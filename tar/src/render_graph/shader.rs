use wgpu::naga::{
    front::wgsl,
    valid::{Capabilities, ValidationFlags, Validator},
};

pub struct ShaderBinding {
    pub set: u32,
    pub binding: u32,
    pub name: String,
    pub resource_type: String,
}

pub struct Shader {
    src: String,
    shader_module: Option<wgpu::ShaderModule>,
    bindings: Vec<ShaderBinding>,
    errors: Vec<String>,
    warnings: Vec<String>,
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
                self.errors.push(format!("Parse error: {}", parse_error));
                return;
            }
        };

        // Validate the module
        let mut validator = Validator::new(ValidationFlags::all(), Capabilities::all());
        let _module_info = match validator.validate(&module) {
            Ok(info) => info,
            Err(validation_error) => {
                self.errors
                    .push(format!("Validation error: {}", validation_error));
                return;
            }
        };

        let mut new_bindings = Vec::new();

        // Extract bindings from the validated module
        for (_handle, global) in module.global_variables.iter() {
            if let Some(binding) = &global.binding {
                let resource_type = match module.types[global.ty].inner {
                    wgpu::naga::TypeInner::Image { dim, .. } => match dim {
                        wgpu::naga::ImageDimension::D1 => "Texture1D",
                        wgpu::naga::ImageDimension::D2 => "Texture2D",
                        wgpu::naga::ImageDimension::D3 => "Texture3D",
                        wgpu::naga::ImageDimension::Cube => "TextureCube",
                    },
                    wgpu::naga::TypeInner::Sampler { .. } => "Sampler",
                    wgpu::naga::TypeInner::Struct { .. } => match global.space {
                        wgpu::naga::AddressSpace::Uniform => "Uniform Buffer",
                        wgpu::naga::AddressSpace::Storage { .. } => "Storage Buffer",
                        _ => "Buffer",
                    },
                    _ => "Unknown",
                };

                new_bindings.push(ShaderBinding {
                    set: binding.group,
                    binding: binding.binding,
                    name: global
                        .name
                        .clone()
                        .unwrap_or_else(|| "unnamed_binding".to_string()),
                    resource_type: resource_type.to_string(),
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

    pub fn get_errors(&self) -> &[String] {
        &self.errors
    }

    pub fn get_warnings(&self) -> &[String] {
        &self.warnings
    }

    pub fn get_source(&self) -> &str {
        &self.src
    }

    pub fn update_source(&mut self, new_src: String, device: &wgpu::Device) {
        if new_src != self.src {
            self.src = new_src;
            self.compile(device);
        }
    }
}
