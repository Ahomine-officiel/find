// Renderer: instanced straw pile, static merged world (barn + outdoors),
// dynamic props, sky, HUD, blit. Everything tuned for low-end GPUs:
// ~1 draw call per system, static buffers, tiny per-frame uploads.

use crate::cam::Camera;

use crate::hud::{Hud, HudQuad};
use crate::mesh::{build_all_meshes, Vertex};
use crate::settings::Settings;
use crate::world::{StrawInst, World, PILE_H, PILE_R};
use bytemuck::Pod;
use bytemuck::Zeroable;
use glam::Mat4;

/// One dynamic prop instance: 80 bytes (mat4 rows + tint/flag).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct PropInst {
    pub m0: [f32; 4],
    pub m1: [f32; 4],
    pub m2: [f32; 4],
    pub m3: [f32; 4],
    pub icol: [f32; 4],
}

impl PropInst {
    pub fn identity() -> Self {
        PropInst {
            m0: [1.0, 0.0, 0.0, 0.0],
            m1: [0.0, 1.0, 0.0, 0.0],
            m2: [0.0, 0.0, 1.0, 0.0],
            m3: [0.0, 0.0, 0.0, 1.0],
            icol: [1.0, 1.0, 1.0, 0.0],
        }
    }
    pub fn from_mat(m: Mat4, col: [f32; 4]) -> Self {
        let c = m.to_cols_array_2d();
        PropInst {
            m0: c[0],
            m1: c[1],
            m2: c[2],
            m3: c[3],
            icol: col,
        }
    }
}

/// Uniform block matching the WGSL `Globals` struct (208 bytes).
#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct Globals {
    pub view_proj: [f32; 16],
    pub cam_pos: [f32; 4],
    pub cam_fwd: [f32; 4],
    pub cam_right: [f32; 4],
    pub cam_up: [f32; 4],
    pub sun_dir: [f32; 4],
    pub fog_color: [f32; 4],
    pub sky_low: [f32; 4],
    pub sky_high: [f32; 4],
    pub params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct HudParams {
    pub params: [f32; 4], // width_px, height_px, ui_scale, time
}


/// All pipelines + shared bind groups. Created once; can be created
/// headlessly (no surface) for validation.
pub struct Pipelines {
    pub globals: wgpu::Buffer,
    pub scene_bind: wgpu::BindGroup,
    pub scene_bgl: wgpu::BindGroupLayout,
    pub sky_pipe: wgpu::RenderPipeline,
    pub prop_pipe: wgpu::RenderPipeline,
    pub straw_pipe: wgpu::RenderPipeline,
    pub hud_pipe: wgpu::RenderPipeline,
    pub blit_pipe: wgpu::RenderPipeline,
    pub hud_bind: wgpu::BindGroup,
    pub hud_params: wgpu::Buffer,
}

pub struct GpuMesh {
    pub vb: wgpu::Buffer,
    pub ib: wgpu::Buffer,
    pub count: u32,
}

pub struct SceneTarget {
    pub color: wgpu::Texture,
    pub color_view: wgpu::TextureView,
    pub depth: wgpu::Texture,
    pub depth_view: wgpu::TextureView,
    pub sampler: wgpu::Sampler,
    pub blit_bind: wgpu::BindGroup,
    pub width: u32,
    pub height: u32,
}

pub struct Renderer {
    pub surface: wgpu::Surface<'static>,
    pub config: wgpu::SurfaceConfiguration,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,

    scene: Option<SceneTarget>,
    pipes: Pipelines,

    // static merged world (barn + outdoors)
    world_mesh: GpuMesh,
    identity_buf: wgpu::Buffer,

    // dynamic prop meshes by id
    prop_meshes: Vec<GpuMesh>,
    prop_inst: wgpu::Buffer,
    prop_inst_cap: usize,

    // straws
    straw_vb: wgpu::Buffer,
    straw_ib: wgpu::Buffer,
    straw_static: wgpu::Buffer,
    straw_static_count: u32,
    straw_dyn: wgpu::Buffer,
    straw_dyn_cap: usize,

    hud_quads: wgpu::Buffer,
    hud_quad_cap: usize,
}

fn mat4_cols(m: Mat4) -> [[f32; 4]; 4] {
    m.to_cols_array_2d()
}

impl Renderer {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: wgpu::Surface<'static>,
        adapter: &wgpu::Adapter,
        window_size: (u32, u32),
        world: &World,
        _settings: &Settings,
    ) -> Self {
        let format = {
            let caps = surface.get_capabilities(adapter);
            caps.formats
                .iter()
                .copied()
                .find(|f| !f.is_srgb())
                .unwrap_or(caps.formats[0])
        };
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width: window_size.0.max(1),
            height: window_size.1.max(1),
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: wgpu::CompositeAlphaMode::Auto,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &config);

impl Pipelines {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, format: wgpu::TextureFormat) -> Self {
        // ---------------- shaders ----------------
        let main_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/main.wgsl").into()),
            label: Some("main"),
        });
        let sky_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/sky.wgsl").into()),
            label: Some("sky"),
        });
        let hud_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/hud.wgsl").into()),
            label: Some("hud"),
        });
        let blit_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            source: wgpu::ShaderSource::Wgsl(include_str!("shaders/blit.wgsl").into()),
            label: Some("blit"),
        });

        // ---------------- globals uniform ----------------
        let globals = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("globals"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let scene_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("scene_bgl"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<Globals>() as u64),
                },
                count: None,
            }],
        });
        let scene_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("scene_bind"),
            layout: &scene_bgl,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: globals.as_entire_binding(),
            }],
        });
        // One SHARED pipeline layout for every scene pipeline: the bind group and all
        // pipelines must reference the exact same BindGroupLayout object, otherwise
        // wgpu rejects the draw (auto-derived layouts never match manual ones).
        let scene_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("scene_pl"),
            bind_group_layouts: &[&scene_bgl],
            push_constant_ranges: &[],
        });

        // ---------------- pipelines ----------------
        let prop_vlayout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64, // 40
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 12, shader_location: 1 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 24, shader_location: 2 },
            ],
        };
        let prop_ilayout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<PropInst>() as u64, // 80
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 0, shader_location: 3 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 16, shader_location: 4 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 32, shader_location: 5 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 48, shader_location: 6 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 64, shader_location: 7 },
            ],
        };
        let straw_vlayout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<Vertex>() as u64, // 40
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 24, shader_location: 2 },
            ],
        };
        let straw_ilayout = wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<StrawInst>() as u64, // 32
            step_mode: wgpu::VertexStepMode::Instance,
            attributes: &[
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 0, shader_location: 3 },
                wgpu::VertexAttribute { format: wgpu::VertexFormat::Uint32x4, offset: 16, shader_location: 4 },
            ],
        };

        let prop_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("prop"),
            layout: Some(&scene_pl),
            vertex: wgpu::VertexState {
                module: &main_mod,
                entry_point: Some("vs_prop"),
                buffers: &[prop_vlayout, prop_ilayout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &main_mod,
                entry_point: Some("fs_prop"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        let straw_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("straw"),
            layout: Some(&scene_pl),
            vertex: wgpu::VertexState {
                module: &main_mod,
                entry_point: Some("vs_straw"),
                buffers: &[straw_vlayout, straw_ilayout],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &main_mod,
                entry_point: Some("fs_straw"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: None, // straws are double-sided flat cards
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::LessEqual,
                stencil: Default::default(),
                bias: wgpu::DepthBiasState {
                    constant: -1,
                    slope_scale: -1.0,
                    clamp: 0.0,
                },
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        let sky_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("sky"),
            layout: Some(&scene_pl),
            vertex: wgpu::VertexState {
                module: &sky_mod,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &sky_mod,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: wgpu::TextureFormat::Depth24Plus,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // HUD
        let hud_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hud_params"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let atlas = crate::hud::build_atlas();
        let font_tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("font"),
            size: wgpu::Extent3d { width: 128, height: 64, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &font_tex,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            &atlas,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(128),
                rows_per_image: Some(64),
            },
            wgpu::Extent3d { width: 128, height: 64, depth_or_array_layers: 1 },
        );
        let font_view = font_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let font_samp = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let hud_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("hud_bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: wgpu::BufferSize::new(std::mem::size_of::<HudParams>() as u64),
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let hud_pl = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("hud_pl"),
            bind_group_layouts: &[&hud_bgl],
            push_constant_ranges: &[],
        });
        let hud_bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("hud_bind"),
            layout: &hud_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: hud_params.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&font_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&font_samp) },
            ],
        });

        let hud_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("hud"),
            layout: Some(&hud_pl),
            vertex: wgpu::VertexState {
                module: &hud_mod,
                entry_point: Some("vs"),
                buffers: &[wgpu::VertexBufferLayout {
                    array_stride: std::mem::size_of::<HudQuad>() as u64, // 48
                    step_mode: wgpu::VertexStepMode::Instance,
                    attributes: &[
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 0, shader_location: 0 },
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 16, shader_location: 1 },
                        wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 32, shader_location: 2 },
                    ],
                }],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &hud_mod,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });

        // blit (bind group uses the pipeline's own layout via get_bind_group_layout)
        let blit_pipe = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("blit"),
            layout: None,
            vertex: wgpu::VertexState {
                module: &blit_mod,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &blit_mod,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: Default::default(),
            multiview: None,
            cache: None,
        });
        Pipelines {
            globals,
            scene_bind,
            scene_bgl,
            sky_pipe,
            prop_pipe,
            straw_pipe,
            hud_pipe,
            blit_pipe,
            hud_bind,
            hud_params,
        }
    }
}

        // ---------------- static world mesh (barn + machines + outdoors merged) ----------------
        let barn = crate::mesh::barn_mesh();
        let machines = crate::mesh::machines_mesh();
        let outdoor = crate::mesh::outdoor_mesh();
        let mut verts = barn.0;
        let mut idx = barn.1;
        let base = verts.len() as u16;
        verts.extend(machines.0);
        for i in machines.1 {
            idx.push(i + base);
        }
        let base = verts.len() as u16;
        verts.extend(outdoor.0);
        for i in outdoor.1 {
            idx.push(i + base);
        }
        assert!(verts.len() < 65535, "world mesh too big for u16 indices");
        let world_mesh = GpuMesh {
            vb: device.create_buffer_init_bfn(&verts),
            ib: device.create_buffer_init_bfn(&idx),
            count: idx.len() as u32,
        };
        let identity_buf = device.create_buffer_init_bfn(&[PropInst::identity()]);

        // ---------------- prop meshes ----------------
        let prop_meshes: Vec<GpuMesh> = build_all_meshes()
            .into_iter()
            .map(|(v, i)| GpuMesh {
                vb: device.create_buffer_init_bfn(&v),
                ib: device.create_buffer_init_bfn(&i),
                count: i.len() as u32,
            })
            .collect();

        let prop_inst_cap: usize = 4096;
        let prop_inst = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("prop_inst"),
            size: (prop_inst_cap * std::mem::size_of::<PropInst>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ---------------- straw buffers ----------------
        let straw_mesh = crate::mesh::StrawMesh::build();
        let straw_vb = device.create_buffer_init_bfn(&straw_mesh.0);
        let straw_ib = device.create_buffer_init_bfn(&straw_mesh.1);
        let straw_static = device.create_buffer_init_bfn(&world.straws);
        let straw_static_count = world.straws.len() as u32;
        let straw_dyn_cap: usize = 8192;
        let straw_dyn = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("straw_dyn"),
            size: (straw_dyn_cap * std::mem::size_of::<StrawInst>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        // ---------------- HUD quad buffer ----------------
        let hud_quad_cap: usize = 16384;
        let hud_quads = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("hud_quads"),
            size: (hud_quad_cap * std::mem::size_of::<HudQuad>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        let pipes = Pipelines::new(&device, &queue, format);

        let mut this = Renderer {
            surface,
            config,
            device,
            queue,
            scene: None,
            pipes,
            world_mesh,
            identity_buf,
            prop_meshes,
            prop_inst,
            prop_inst_cap,
            straw_vb,
            straw_ib,
            straw_static,
            straw_static_count,
            straw_dyn,
            straw_dyn_cap,
            hud_quads,
            hud_quad_cap,
        };
        this.resize(window_size.0, window_size.1, 1.0);
        this
    }

    pub fn resize(&mut self, w: u32, h: u32, scale: f32) {
        let w = (w.max(1) as f32 * scale).round().max(1.0) as u32;
        let h = (h.max(1) as f32 * scale).round().max(1.0) as u32;
        self.config.width = w.max(1);
        self.config.height = h.max(1);
        self.surface.configure(&self.device, &self.config);
        let scene_w = w.max(1);
        let scene_h = h.max(1);

        let color = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene_color"),
            size: wgpu::Extent3d { width: scene_w, height: scene_h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: self.config.format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING,
            view_formats: &[],
        });
        let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());
        let depth = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene_depth"),
            size: wgpu::Extent3d { width: scene_w, height: scene_h, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = self.device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let blit_bgl = self.pipes.blit_pipe.get_bind_group_layout(0);
        let blit_bind = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("blit_bind"),
            layout: &blit_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(&color_view) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });
        self.scene = Some(SceneTarget {
            color,
            color_view,
            depth,
            depth_view,
            sampler,
            blit_bind,
            width: scene_w,
            height: scene_h,
        });

    }

    pub fn set_present_mode(&mut self, vsync: bool) {
        self.config.present_mode = if vsync {
            wgpu::PresentMode::Fifo
        } else {
            wgpu::PresentMode::Immediate
        };
        self.surface.configure(&self.device, &self.config);
    }

    /// Full frame draw. `prop_batches`: (mesh_id, first_instance, count).
    #[allow(clippy::too_many_arguments)]
    pub fn draw_frame(
        &mut self,
        cam: &Camera,
        aspect: (f32, f32),
        time: f32,
        cloud_amt: f32,
        straw_dyn: &[StrawInst],
        prop_insts: &[PropInst],
        prop_batches: &[(usize, u32, u32)],
        hud: &Hud,
    ) {
        let Some(scene) = self.scene.as_ref() else { return };

        // --- update globals ---
        let vp = cam.view_proj(aspect.0 / aspect.1, cam.near(), 900.0);
        let fwd = cam.fwd();
        let right = cam.right();
        let up = right.cross(fwd).normalize_or_zero();
        let globals = Globals {
            view_proj: vp.to_cols_array(),
            cam_pos: [cam.pos.x, cam.pos.y, cam.pos.z, 0.0],
            cam_fwd: [fwd.x, fwd.y, fwd.z, (cam.effective_fov() * 0.5).tan()],
            cam_right: [right.x, right.y, right.z, 0.0],
            cam_up: [up.x, up.y, up.z, 0.0],
            sun_dir: [0.45, 0.58, 0.28, 0.0],
            fog_color: [0.72, 0.80, 0.88, 0.0],
            sky_low: [0.64, 0.76, 0.88, 0.0],
            sky_high: [0.30, 0.52, 0.82, 0.0],
            params: [time, 120.0, 700.0, cloud_amt],
        };
        self.queue.write_buffer(&self.pipes.globals, 0, bytemuck::bytes_of(&globals));
        let hp = HudParams {
            params: [aspect.0, aspect.1, 1.0, time],
        };
        self.queue.write_buffer(&self.pipes.hud_params, 0, bytemuck::bytes_of(&hp));

        // --- per-frame instance uploads ---
        if !straw_dyn.is_empty() {
            let n = straw_dyn.len().min(self.straw_dyn_cap);
            self.queue.write_buffer(&self.straw_dyn, 0, bytemuck::cast_slice(&straw_dyn[..n]));
        }
        if !prop_insts.is_empty() {
            let n = prop_insts.len().min(self.prop_inst_cap);
            self.queue.write_buffer(&self.prop_inst, 0, bytemuck::cast_slice(&prop_insts[..n]));
        }
        if !hud.quads.is_empty() {
            let n = hud.quads.len().min(self.hud_quad_cap);
            self.queue.write_buffer(&self.hud_quads, 0, bytemuck::cast_slice(&hud.quads[..n]));
        }

        let frame = match self.surface.get_current_texture() {
            Ok(f) => f,
            Err(_) => return,
        };
        let frame_view = frame.texture.create_view(&wgpu::TextureViewDescriptor::default());
        let mut enc = self.device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });

        {
            let mut pass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &scene.color_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.30, g: 0.52, b: 0.82, a: 1.0 }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                    view: &scene.depth_view,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // sky
            pass.set_pipeline(&self.pipes.sky_pipe);
            pass.set_bind_group(0, &self.pipes.scene_bind, &[]);
            pass.draw(0..3, 0..1);

            // static merged world: 1 draw call
            pass.set_pipeline(&self.pipes.prop_pipe);
            pass.set_bind_group(0, &self.pipes.scene_bind, &[]);
            pass.set_vertex_buffer(0, self.world_mesh.vb.slice(..));
            pass.set_vertex_buffer(1, self.identity_buf.slice(..));
            pass.set_index_buffer(self.world_mesh.ib.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..self.world_mesh.count, 0, 0..1);

            // dynamic props
            for (mesh_id, first, count) in prop_batches {
                if *mesh_id >= self.prop_meshes.len() || *count == 0 {
                    continue;
                }
                let m = &self.prop_meshes[*mesh_id];
                let byte_off = (*first as u64) * std::mem::size_of::<PropInst>() as u64;
                pass.set_vertex_buffer(0, m.vb.slice(..));
                pass.set_vertex_buffer(1, self.prop_inst.slice(byte_off..));
                pass.set_index_buffer(m.ib.slice(..), wgpu::IndexFormat::Uint16);
                pass.draw_indexed(0..m.count, 0, 0..*count);
            }

            // straws: static pile
            pass.set_pipeline(&self.pipes.straw_pipe);
            pass.set_bind_group(0, &self.pipes.scene_bind, &[]);
            pass.set_vertex_buffer(0, self.straw_vb.slice(..));
            pass.set_vertex_buffer(1, self.straw_static.slice(..));
            pass.set_index_buffer(self.straw_ib.slice(..), wgpu::IndexFormat::Uint16);
            pass.draw_indexed(0..6, 0, 0..self.straw_static_count);

            // dynamic straws (tossed/flying)
            if !straw_dyn.is_empty() {
                let n = (straw_dyn.len().min(self.straw_dyn_cap)) as u32;
                pass.set_vertex_buffer(1, self.straw_dyn.slice(..));
                pass.draw_indexed(0..6, 0, 0..n);
            }

            // HUD
            if !hud.quads.is_empty() {
                let n = hud.quads.len().min(self.hud_quad_cap) as u32;
                pass.set_pipeline(&self.pipes.hud_pipe);
                pass.set_bind_group(0, &self.pipes.hud_bind, &[]);
                pass.set_vertex_buffer(0, self.hud_quads.slice(..));
                pass.draw(0..6, 0..n);
            }
        }

        // blit scene -> swapchain
        {
            let mut bpass = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &frame_view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            bpass.set_pipeline(&self.pipes.blit_pipe);
            bpass.set_bind_group(0, &scene.blit_bind, &[]);
            bpass.draw(0..3, 0..1);
        }

        self.queue.submit([enc.finish()]);
        frame.present();
    }

    pub fn pile_dims() -> (f32, f32) {
        (PILE_R, PILE_H)
    }

    /// Swap the static straw instance buffer (quality change / new seed).
    pub fn replace_straw_static(&mut self, world: &World) {
        self.straw_static = self.device.create_buffer_init_bfn(&world.straws);
        self.straw_static_count = world.straws.len() as u32;
    }
}

// small extension so we don't pull in the whole BufferInitDescriptor dance
trait CreateBufferInit {
    fn create_buffer_init_bfn<T: Pod>(&self, data: &[T]) -> wgpu::Buffer;
}

impl CreateBufferInit for wgpu::Device {
    fn create_buffer_init_bfn<T: Pod>(&self, data: &[T]) -> wgpu::Buffer {
        let bytes = bytemuck::cast_slice(data);
        self.create_buffer(&wgpu::BufferDescriptor {
            label: None,
            size: bytes.len() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: true,
        })
        .with_data(bytes)
    }
}

trait WithData {
    fn with_data(self, bytes: &[u8]) -> wgpu::Buffer;
}

impl WithData for wgpu::Buffer {
    fn with_data(self, bytes: &[u8]) -> wgpu::Buffer {
        if !bytes.is_empty() {
            self.slice(..).get_mapped_range_mut().copy_from_slice(bytes);
            self.unmap();
        }
        self
    }
}
