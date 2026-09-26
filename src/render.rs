// Renderer: instanced straw pile (1 draw call), Blender-model props, sky, HUD, blit.
// Tuned for low-end GPUs: static straw buffer, tiny per-frame uploads, one offscreen
// target (render_scale + MSAA), single blit to swapchain.
use crate::cam::Camera;
use crate::hud::{Hud, HudQuad};
use crate::mesh;
use crate::model::{Model, ModelVertex};
use crate::settings::Settings;
use crate::shaders;
use crate::world::{StrawInst, World};
use bytemuck::Pod;
use bytemuck::Zeroable;
use glam::{Mat4, Vec3};

pub struct PropDraw {
    pub model: usize,
    pub mat: Mat4,
    pub col: [f32; 3],
    pub flag: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Globals {
    view_proj: [[f32; 4]; 4],
    cam_pos: [f32; 4],
    cam_fwd: [f32; 4],
    cam_right: [f32; 4],
    cam_up: [f32; 4],
    sun_dir: [f32; 4],
    fog_color: [f32; 4],
    sky_low: [f32; 4],
    sky_high: [f32; 4],
    params: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PropInst {
    m0: [f32; 4],
    m1: [f32; 4],
    m2: [f32; 4],
    m3: [f32; 4],
    icol: [f32; 4],
}

const SUN_DIR: Vec3 = Vec3::new(-0.42, 0.74, 0.36);
const SKY_HIGH: [f32; 4] = [0.34, 0.56, 0.88, 1.0];
const SKY_LOW: [f32; 4] = [0.74, 0.82, 0.90, 1.0];
const FOG: [f32; 4] = [0.78, 0.84, 0.90, 1.0];
const FOG_NEAR: f32 = 70.0;
const FOG_FAR: f32 = 460.0;

pub struct Frame<'a> {
    pub cam: &'a Camera,
    pub hud: &'a Hud,
    pub props: &'a [PropDraw],
    pub vm: Option<&'a PropDraw>,
    pub toss: &'a [StrawInst],
    pub time: f32,
    pub ui_scale: f32,
}

struct Targets {
    color: wgpu::TextureView,
    color_tex: wgpu::Texture,
    resolve: Option<wgpu::TextureView>,
    resolve_tex: Option<wgpu::Texture>,
    depth: wgpu::TextureView,
    vm_depth: wgpu::TextureView,
    sampler: wgpu::Sampler,
    blit_bg: wgpu::BindGroup,
    size: (u32, u32),
}

pub struct Renderer {
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    surface: Option<wgpu::Surface<'static>>,
    config: wgpu::SurfaceConfiguration,
    size: (u32, u32),
    pending_shot: Option<(wgpu::Buffer, (u32, u32), String)>,

    sky_pl: wgpu::RenderPipeline,
    straw_pl: wgpu::RenderPipeline,
    prop_pl: wgpu::RenderPipeline,
    hud_pl: wgpu::RenderPipeline,
    blit_pl: wgpu::RenderPipeline,
    blit_bgl: wgpu::BindGroupLayout,

    scene_bg: wgpu::BindGroup,
    vm_bg: wgpu::BindGroup,
    scene_ubo: wgpu::Buffer,
    vm_ubo: wgpu::Buffer,

    hud_bg: wgpu::BindGroup,
    hud_ubo: wgpu::Buffer,
    quad_buf: wgpu::Buffer,
    quad_cap: u64,

    straw_mesh: (wgpu::Buffer, wgpu::Buffer, u32),
    straw_inst: Option<wgpu::Buffer>,
    straw_inst_cap: u64,
    toss_buf: wgpu::Buffer,
    toss_cap: u64,

    prop_meshes: Vec<(wgpu::Buffer, wgpu::Buffer, u32)>,
    prop_inst_buf: wgpu::Buffer,
    prop_inst_cap: u64,

    msaa: u32,
    scale: f32,
    targets: Option<Targets>,
}

fn vb_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<ModelVertex>() as u64,
        step_mode: wgpu::VertexStepMode::Vertex,
        attributes: &[
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 0, shader_location: 0 },
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 12, shader_location: 1 },
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x3, offset: 24, shader_location: 2 },
        ],
    }
}

fn straw_inst_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<StrawInst>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 0, shader_location: 3 },
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Uint32x4, offset: 16, shader_location: 4 },
        ],
    }
}

fn prop_inst_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<PropInst>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 0, shader_location: 3 },
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 16, shader_location: 4 },
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 32, shader_location: 5 },
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 48, shader_location: 6 },
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 64, shader_location: 7 },
        ],
    }
}

fn quad_layout() -> wgpu::VertexBufferLayout<'static> {
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<HudQuad>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &[
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 0, shader_location: 0 },
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 16, shader_location: 1 },
            wgpu::VertexAttribute { format: wgpu::VertexFormat::Float32x4, offset: 32, shader_location: 2 },
        ],
    }
}

impl Renderer {
    pub fn new(
        device: wgpu::Device,
        queue: wgpu::Queue,
        surface: Option<wgpu::Surface<'static>>,
        config: wgpu::SurfaceConfiguration,
        settings: &Settings,
        models: Vec<Model>,
    ) -> Self {
        let msaa = settings.quality.msaa();
        let scale = settings.render_scale;

        let scene_ubo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ubo_scene"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let vm_ubo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ubo_vm"),
            size: std::mem::size_of::<Globals>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let scene_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bgl_scene"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let scene_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bg_scene"),
            layout: &scene_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: scene_ubo.as_entire_binding() }],
        });
        let vm_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bg_vm"),
            layout: &scene_bgl,
            entries: &[wgpu::BindGroupEntry { binding: 0, resource: vm_ubo.as_entire_binding() }],
        });

        // HUD resources
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
            wgpu::TexelCopyBufferLayout { offset: 0, bytes_per_row: Some(128), rows_per_image: Some(64) },
            wgpu::Extent3d { width: 128, height: 64, depth_or_array_layers: 1 },
        );
        let font_view = font_tex.create_view(&wgpu::TextureViewDescriptor::default());
        let font_samp = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let hud_ubo = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("ubo_hud"),
            size: 16,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let quad_cap = 24576u64;
        let quad_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("quads"),
            size: quad_cap * std::mem::size_of::<HudQuad>() as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let hud_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bgl_hud"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: false },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::NonFiltering),
                    count: None,
                },
            ],
        });
        let hud_bg = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bg_hud"),
            layout: &hud_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: hud_ubo.as_entire_binding() },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::TextureView(&font_view) },
                wgpu::BindGroupEntry { binding: 2, resource: wgpu::BindingResource::Sampler(&font_samp) },
            ],
        });

        // blit layout
        let blit_bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("bgl_blit"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });

        // shaders
        let scene_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("main"),
            source: wgpu::ShaderSource::Wgsl(shaders::MAIN_WGSL.into()),
        });
        let sky_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("sky"),
            source: wgpu::ShaderSource::Wgsl(shaders::SKY_WGSL.into()),
        });
        let hud_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("hud"),
            source: wgpu::ShaderSource::Wgsl(shaders::HUD_WGSL.into()),
        });
        let blit_mod = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("blit"),
            source: wgpu::ShaderSource::Wgsl(shaders::BLIT_WGSL.into()),
        });

        let pl_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pl_scene"),
            bind_group_layouts: &[&scene_bgl],
            push_constant_ranges: &[],
        });
        let hud_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pl_hud"),
            bind_group_layouts: &[&hud_bgl],
            push_constant_ranges: &[],
        });
        let blit_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("pl_blit"),
            bind_group_layouts: &[&blit_bgl],
            push_constant_ranges: &[],
        });

        let targets_fmt = wgpu::TextureFormat::Bgra8UnormSrgb;
        let depth_fmt = wgpu::TextureFormat::Depth24Plus;

        let sky_pl = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pl_sky"),
            layout: Some(&pl_layout),
            vertex: wgpu::VertexState {
                module: &sky_mod,
                entry_point: Some("vs"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &sky_mod,
                entry_point: Some("fs"),
                targets: &[Some(targets_fmt.into())],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_fmt,
                depth_write_enabled: false,
                depth_compare: wgpu::CompareFunction::Always,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let straw_pl = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pl_straw"),
            layout: Some(&pl_layout),
            vertex: wgpu::VertexState {
                module: &scene_mod,
                entry_point: Some("vs_straw"),
                buffers: &[vb_layout(), straw_inst_layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &scene_mod,
                entry_point: Some("fs_straw"),
                targets: &[Some(targets_fmt.into())],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState { cull_mode: None, ..Default::default() },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_fmt,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState::default(),
            }),
            multisample: wgpu::MultisampleState { count: msaa, ..Default::default() },
            multiview: None,
            cache: None,
        });

        let prop_pl = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pl_prop"),
            layout: Some(&pl_layout),
            vertex: wgpu::VertexState {
                module: &scene_mod,
                entry_point: Some("vs_prop"),
                buffers: &[vb_layout(), prop_inst_layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &scene_mod,
                entry_point: Some("fs_prop"),
                targets: &[Some(targets_fmt.into())],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                cull_mode: Some(wgpu::Face::Back),
                ..Default::default()
            },
            depth_stencil: Some(wgpu::DepthStencilState {
                format: depth_fmt,
                depth_write_enabled: true,
                depth_compare: wgpu::CompareFunction::Less,
                stencil: wgpu::StencilState::default(),
                bias: wgpu::DepthBiasState { constant: 1, slope_scale: 1.0, clamp: 0.0 },
            }),
            multisample: wgpu::MultisampleState { count: msaa, ..Default::default() },
            multiview: None,
            cache: None,
        });

        let hud_pl = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pl_hud"),
            layout: Some(&hud_layout),
            vertex: wgpu::VertexState {
                module: &hud_mod,
                entry_point: Some("vs"),
                buffers: &[quad_layout()],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &hud_mod,
                entry_point: Some("fs"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: targets_fmt,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleStrip,
                ..Default::default()
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        let blit_pl = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("pl_blit"),
            layout: Some(&blit_layout),
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
                    format: config.format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        // meshes
        let sm = mesh::StrawMesh::build();
        let sm_verts: Vec<ModelVertex> = sm
            .0
            .iter()
            .map(|v| ModelVertex { pos: v.pos, nrm: v.nrm, col: v.col })
            .collect();
        let sm_idx: Vec<u32> = sm.1.iter().map(|&i| i as u32).collect();
        let straw_mesh = upload_mesh(&device, &queue, &sm_verts, &sm_idx);
        let prop_meshes = models
            .iter()
            .map(|m| upload_mesh(&device, &queue, &m.verts, &m.idx))
            .collect();

        let toss_cap = 4096u64;
        let toss_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("toss"),
            size: toss_cap * 32,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let prop_inst_cap = 192u64;
        let prop_inst_buf = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("prop_inst"),
            size: prop_inst_cap * 80,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });

        Renderer {
            device,
            queue,
            surface,
            config,
            size: (0, 0),
            pending_shot: None,
            sky_pl,
            straw_pl,
            prop_pl,
            hud_pl,
            blit_pl,
            blit_bgl,
            scene_bg,
            vm_bg,
            scene_ubo,
            vm_ubo,
            hud_bg,
            hud_ubo,
            quad_buf,
            quad_cap,
            straw_mesh,
            straw_inst: None,
            straw_inst_cap: 0,
            toss_buf,
            toss_cap,
            prop_meshes,
            prop_inst_buf,
            prop_inst_cap,
            msaa,
            scale,
            targets: None,
        }
    }

    pub fn set_straw_instances(&mut self, world: &World) {
        let n = world.straws.len() as u64;
        let bytes: &[u8] = bytemuck::cast_slice(&world.straws);
        match &self.straw_inst {
            Some(b) if self.straw_inst_cap >= n => {
                self.queue.write_buffer(b, 0, bytes);
            }
            _ => {
                let b = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("straw_inst"),
                    size: n * 32,
                    usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                self.queue.write_buffer(&b, 0, bytes);
                self.straw_inst = Some(b);
                self.straw_inst_cap = n;
            }
        }
    }

    pub fn update_straw_states(&mut self, indices: &[u32]) {
        let Some(buf) = &self.straw_inst else { return };
        for &i in indices {
            let off = (i as u64) * 32 + 24;
            let one: [u8; 4] = 1u32.to_le_bytes();
            self.queue.write_buffer(buf, off, &one);
        }
    }

    pub fn resize(&mut self, w: u32, h: u32) {
        if w == 0 || h == 0 {
            return;
        }
        self.size = (w, h);
        self.config.width = w;
        self.config.height = h;
        if let Some(sfc) = &self.surface {
            sfc.configure(&self.device, &self.config);
        }
        self.rebuild_targets();
    }

    /// Headless helper: force target size without a surface.
    pub fn set_headless_size(&mut self, w: u32, h: u32) {
        self.size = (w, h);
        self.config.width = w;
        self.config.height = h;
        self.rebuild_targets();
    }

    pub fn request_screenshot(&mut self, path: &str) {
        if self.pending_shot.is_none() {
            if let Some(t) = self.targets.as_ref() {
                let (sw, sh) = t.size;
                let buf = self.device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("shot"),
                    size: (sw * sh * 4) as u64,
                    usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                self.pending_shot = Some((buf, (sw, sh), path.to_string()));
            }
        }
    }

    pub fn finish_screenshot(&mut self) {
        let Some((buf, (sw, sh), path)) = self.pending_shot.take() else { return };
        let (tx, rx) = std::sync::mpsc::channel();
        let slice = buf.slice(..);
        slice.map_async(wgpu::MapMode::Read, move |r| {
            let _ = tx.send(r);
        });
        self.device.poll(wgpu::Maintain::Wait);
        if rx.recv().is_ok() {
            let data = slice.get_mapped_range().to_vec();
            drop(slice);
            buf.unmap();
            let mut pixels = Vec::with_capacity(data.len());
            for px in data.chunks_exact(4) {
                pixels.push(px[2]);
                pixels.push(px[1]);
                pixels.push(px[0]);
                pixels.push(px[3]);
            }
            let img = image::RgbaImage::from_raw(sw, sh, pixels).unwrap();
            let _ = img.save(&path);
            println!("screenshot saved: {path}");
        }
    }

    pub fn apply_quality(&mut self, settings: &Settings) {
        self.msaa = settings.quality.msaa();
        self.scale = settings.render_scale;
        self.rebuild_targets();
    }

    fn rebuild_targets(&mut self) {
        let (w, h) = self.size;
        if w == 0 || h == 0 {
            return;
        }
        let sw = ((w as f32 * self.scale) as u32).max(64);
        let sh = ((h as f32 * self.scale) as u32).max(64);
        let device = &self.device;
        let color = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene_color"),
            size: wgpu::Extent3d { width: sw, height: sh, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: self.msaa,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Bgra8UnormSrgb,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let resolve = if self.msaa > 1 {
            Some(device.create_texture(&wgpu::TextureDescriptor {
                label: Some("scene_resolve"),
                size: wgpu::Extent3d { width: sw, height: sh, depth_or_array_layers: 1 },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Bgra8UnormSrgb,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            }))
        } else {
            None
        };
        let depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("scene_depth"),
            size: wgpu::Extent3d { width: sw, height: sh, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: self.msaa,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let vm_depth = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("vm_depth"),
            size: wgpu::Extent3d { width: sw, height: sh, depth_or_array_layers: 1 },
            mip_level_count: 1,
            sample_count: self.msaa,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Depth24Plus,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        });
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let color_view = color.create_view(&wgpu::TextureViewDescriptor::default());
        let depth_view = depth.create_view(&wgpu::TextureViewDescriptor::default());
        let vm_depth_view = vm_depth.create_view(&wgpu::TextureViewDescriptor::default());
        let resolve_view = resolve.as_ref().map(|t| t.create_view(&wgpu::TextureViewDescriptor::default()));
        let src_view_ref = resolve_view.as_ref().unwrap_or(&color_view);
        let blit_bg = self.device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("bg_blit"),
            layout: &self.blit_bgl,
            entries: &[
                wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::TextureView(src_view_ref) },
                wgpu::BindGroupEntry { binding: 1, resource: wgpu::BindingResource::Sampler(&sampler) },
            ],
        });
        self.targets = Some(Targets {
            color: color_view,
            color_tex: color,
            resolve: resolve_view,
            resolve_tex: resolve,
            depth: depth_view,
            vm_depth: vm_depth_view,
            sampler,
            blit_bg,
            size: (sw, sh),
        });
    }

    pub fn render(&mut self, frame: &Frame) -> Result<(), wgpu::SurfaceError> {
        let Some(t) = self.targets.as_ref() else { return Ok(()) };
        let (sw, sh) = t.size;

        // ---- globals ----
        let aspect = sw as f32 / sh as f32;
        let near = frame.cam.near();
        let vp = frame.cam.view_proj(aspect, near, 600.0);
        let tanf = (frame.cam.effective_fov() * 0.5).tan();
        let g = Globals {
            view_proj: vp.to_cols_array_2d(),
            cam_pos: [frame.cam.pos.x, frame.cam.pos.y, frame.cam.pos.z, tanf],
            cam_fwd: ext(frame.cam.fwd(), 0.0),
            cam_right: ext(frame.cam.right(), 0.0),
            cam_up: ext(frame.cam.up(), 0.0),
            sun_dir: ext(SUN_DIR.normalize(), 0.0),
            fog_color: FOG,
            sky_low: SKY_LOW,
            sky_high: SKY_HIGH,
            params: [frame.time, FOG_NEAR, FOG_FAR, 0.85],
        };
        self.queue.write_buffer(&self.scene_ubo, 0, bytemuck::bytes_of(&g));

        let gv = Globals {
            view_proj: Mat4::perspective_rh(frame.cam.effective_fov(), aspect, near, 12.0)
                .to_cols_array_2d(),
            cam_pos: [0.0; 4],
            cam_fwd: [0.0, 0.0, -1.0, 0.0],
            cam_right: [1.0, 0.0, 0.0, 0.0],
            cam_up: [0.0, 1.0, 0.0, 0.0],
            sun_dir: [0.4, 0.8, 0.45, 0.0],
            fog_color: FOG,
            sky_low: SKY_LOW,
            sky_high: SKY_HIGH,
            params: [frame.time, 999.0, 1000.0, 0.0],
        };
        self.queue.write_buffer(&self.vm_ubo, 0, bytemuck::bytes_of(&gv));

        // ---- prop instances ----
        let mut insts: Vec<PropInst> = Vec::with_capacity(frame.props.len() + 1);
        for p in frame.props {
            insts.push(PropInst {
                m0: p.mat.x_axis.to_array(),
                m1: p.mat.y_axis.to_array(),
                m2: p.mat.z_axis.to_array(),
                m3: p.mat.w_axis.to_array(),
                icol: [p.col[0], p.col[1], p.col[2], p.flag],
            });
        }
        if !insts.is_empty() && insts.len() <= self.prop_inst_cap as usize {
            let bytes: &[u8] = bytemuck::cast_slice(&insts);
            self.queue.write_buffer(&self.prop_inst_buf, 0, bytes);
        }
        if !frame.toss.is_empty() {
            let n = frame.toss.len().min(self.toss_cap as usize);
            self.queue.write_buffer(&self.toss_buf, 0, bytemuck::cast_slice(&frame.toss[..n]));
        }
        let quads: &[HudQuad] = &frame.hud.quads;
        let nq = (quads.len() as u64).min(self.quad_cap) as usize;
        if nq > 0 {
            self.queue.write_buffer(&self.quad_buf, 0, bytemuck::cast_slice(&quads[..nq]));
        }
        let hp = [sw as f32, sh as f32, frame.ui_scale, frame.time];
        self.queue.write_buffer(&self.hud_ubo, 0, bytemuck::cast_slice(&hp));

        // ---- passes ----
        let headless = self.surface.is_none();
        let out_view_holder: Option<(wgpu::SurfaceTexture, wgpu::TextureView)>;
        let out_view: &wgpu::TextureView;
        if !headless {
            let ft = self.surface.as_ref().unwrap().get_current_texture()?;
            let v = ft.texture.create_view(&wgpu::TextureViewDescriptor::default());
            out_view = {
                out_view_holder = Some((ft, v));
                &out_view_holder.as_ref().unwrap().1
            };
        } else {
            out_view_holder = None;
            out_view = &t.color; // unused in headless
        }
        let mut enc = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor { label: Some("frame") });
        {
            let color_att = wgpu::RenderPassColorAttachment {
                view: &t.color,
                resolve_target: t.resolve.as_ref(),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color { r: 0.5, g: 0.7, b: 0.9, a: 1.0 }),
                    store: wgpu::StoreOp::Store,
                },
            };
            let depth_att = wgpu::RenderPassDepthStencilAttachment {
                view: &t.depth,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Store,
                }),
                stencil_ops: None,
            };
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("scene"),
                color_attachments: &[Some(color_att)],
                depth_stencil_attachment: Some(depth_att),
                timestamp_writes: None,
                occlusion_query_set: None,
            });

            // sky
            rp.set_pipeline(&self.sky_pl);
            rp.set_bind_group(0, &self.scene_bg, &[]);
            rp.draw(0..3, 0..1);

            // props: contiguous runs per model (main sorts by model)
            if !insts.is_empty() && insts.len() <= self.prop_inst_cap as usize {
                rp.set_pipeline(&self.prop_pl);
                rp.set_bind_group(0, &self.scene_bg, &[]);
                rp.set_vertex_buffer(1, self.prop_inst_buf.slice(..));
                let mut i = 0usize;
                while i < frame.props.len() {
                    let m = frame.props[i].model;
                    let start = i;
                    while i < frame.props.len() && frame.props[i].model == m {
                        i += 1;
                    }
                    let (vb, ib, ni) = &self.prop_meshes[m];
                    rp.set_vertex_buffer(0, vb.slice(..));
                    rp.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                    rp.draw_indexed(0..*ni, 0, start as u32..i as u32);
                }
            }

            // straws
            if let Some(sbuf) = self.straw_inst.as_ref() {
                rp.set_pipeline(&self.straw_pl);
                rp.set_bind_group(0, &self.scene_bg, &[]);
                rp.set_vertex_buffer(0, self.straw_mesh.0.slice(..));
                rp.set_vertex_buffer(1, sbuf.slice(..));
                rp.set_index_buffer(self.straw_mesh.1.slice(..), wgpu::IndexFormat::Uint32);
                rp.draw_indexed(0..self.straw_mesh.2, 0, 0..self.straw_inst_cap as u32);
            }
            // tossed straws (pipeline + mesh still bound)
            if !frame.toss.is_empty() {
                let n = frame.toss.len().min(self.toss_cap as usize) as u32;
                rp.set_vertex_buffer(1, self.toss_buf.slice(..));
                rp.draw_indexed(0..self.straw_mesh.2, 0, 0..n);
            }
        }

        // viewmodel pass (fresh depth so held tools never clip into the pile)
        if let Some(vm) = frame.vm {
            if vm.model < self.prop_meshes.len() {
                let color_att = wgpu::RenderPassColorAttachment {
                    view: &t.color,
                    resolve_target: t.resolve.as_ref(),
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                };
                let depth_att = wgpu::RenderPassDepthStencilAttachment {
                    view: &t.vm_depth,
                    depth_ops: Some(wgpu::Operations {
                        load: wgpu::LoadOp::Clear(1.0),
                        store: wgpu::StoreOp::Store,
                    }),
                    stencil_ops: None,
                };
                let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("viewmodel"),
                    color_attachments: &[Some(color_att)],
                    depth_stencil_attachment: Some(depth_att),
                    timestamp_writes: None,
                    occlusion_query_set: None,
                });
                let (vb, ib, ni) = &self.prop_meshes[vm.model];
                let inst = PropInst {
                    m0: vm.mat.x_axis.to_array(),
                    m1: vm.mat.y_axis.to_array(),
                    m2: vm.mat.z_axis.to_array(),
                    m3: vm.mat.w_axis.to_array(),
                    icol: [vm.col[0], vm.col[1], vm.col[2], vm.flag],
                };
                self.queue.write_buffer(&self.prop_inst_buf, 80, bytemuck::bytes_of(&inst));
                rp.set_pipeline(&self.prop_pl);
                rp.set_bind_group(0, &self.vm_bg, &[]);
                rp.set_vertex_buffer(0, vb.slice(..));
                rp.set_vertex_buffer(1, self.prop_inst_buf.slice(80..160));
                rp.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint32);
                rp.draw_indexed(0..*ni, 0, 0..1);
            }
        }

        // HUD
        {
            let color_att = wgpu::RenderPassColorAttachment {
                view: &t.color,
                resolve_target: t.resolve.as_ref(),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            };
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("hud"),
                color_attachments: &[Some(color_att)],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
            });
            if nq > 0 {
                rp.set_pipeline(&self.hud_pl);
                rp.set_bind_group(0, &self.hud_bg, &[]);
                rp.set_vertex_buffer(0, self.quad_buf.slice(..));
                rp.draw(0..4, 0..nq as u32);
            }
        }

        // blit to swapchain (or capture in headless)
        if !headless {
            let mut rp = enc.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("blit"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: out_view,
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
            rp.set_pipeline(&self.blit_pl);
            rp.set_bind_group(0, &t.blit_bg, &[]);
            rp.draw(0..3, 0..1);
        } else if let Some((ref buf, (bw, bh), _)) = self.pending_shot {
            let src = t.resolve_tex.as_ref().unwrap_or(&t.color_tex);
            enc.copy_texture_to_buffer(
                src.as_image_copy(),
                wgpu::TexelCopyBufferInfo {
                    buffer: buf,
                    layout: wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(bw * 4),
                        rows_per_image: Some(bh),
                    },
                },
                wgpu::Extent3d { width: bw, height: bh, depth_or_array_layers: 1 },
            );
        }
        self.queue.submit(Some(enc.finish()));
        if let Some((ft, _)) = out_view_holder {
            ft.present();
        }
        Ok(())
    }
}

fn ext(v: Vec3, w: f32) -> [f32; 4] {
    [v.x, v.y, v.z, w]
}

fn upload_mesh(device: &wgpu::Device, queue: &wgpu::Queue, verts: &[ModelVertex], idx: &[u32]) -> (wgpu::Buffer, wgpu::Buffer, u32) {
    let vb = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mesh_vb"),
        size: (std::mem::size_of::<ModelVertex>() * verts.len().max(1)) as u64,
        usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&vb, 0, bytemuck::cast_slice(verts));
    let ib = device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("mesh_ib"),
        size: (4 * idx.len().max(1)) as u64,
        usage: wgpu::BufferUsages::INDEX | wgpu::BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    queue.write_buffer(&ib, 0, bytemuck::cast_slice(idx));
    (vb, ib, idx.len() as u32)
}
