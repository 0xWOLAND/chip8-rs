use crate::compiler;
use bytemuck::{Pod, Zeroable};
use std::error::Error;
use std::io::Write;
use std::path::{Path, PathBuf};
use wgpu::util::DeviceExt;

pub type AppResult<T> = Result<T, Box<dyn Error + Send + Sync>>;
pub const DEFAULT_FRAMES: u32 = 300;
const WIDTH: usize = 64;
const HEIGHT: usize = 32;
const PROGRAM_START: usize = 0x200;
const FONT_START: usize = 0x050;

const FONT: [u8; 80] = [
    0xF0, 0x90, 0x90, 0x90, 0xF0, 0x20, 0x60, 0x20, 0x20, 0x70, 0xF0, 0x10, 0xF0, 0x80, 0xF0, 0xF0,
    0x10, 0xF0, 0x10, 0xF0, 0x90, 0x90, 0xF0, 0x10, 0x10, 0xF0, 0x80, 0xF0, 0x10, 0xF0, 0xF0, 0x80,
    0xF0, 0x90, 0xF0, 0xF0, 0x10, 0x20, 0x40, 0x40, 0xF0, 0x90, 0xF0, 0x90, 0xF0, 0xF0, 0x90, 0xF0,
    0x10, 0xF0, 0xF0, 0x90, 0xF0, 0x90, 0x90, 0xE0, 0x90, 0xE0, 0x90, 0xE0, 0xF0, 0x80, 0x80, 0x80,
    0xF0, 0xE0, 0x90, 0x90, 0x90, 0xE0, 0xF0, 0x80, 0xF0, 0x80, 0xF0, 0xF0, 0x80, 0xF0, 0x80, 0x80,
];

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct VmState {
    pub block_id: u32,
    pub i_reg: u32,
    pub sp: u32,
    pub delay_timer: u32,
    pub sound_timer: u32,
    pub rng_state: u32,
    pub v: [u32; 4],
    pub stack: [u32; 8],
    pub memory: [u32; 1024],
}

impl VmState {
    pub fn from_rom(rom: &[u8]) -> Self {
        let mut memory = [0u8; 4096];
        memory[FONT_START..FONT_START + FONT.len()].copy_from_slice(&FONT);
        let program_len = rom.len().min(memory.len().saturating_sub(PROGRAM_START));
        memory[PROGRAM_START..PROGRAM_START + program_len].copy_from_slice(&rom[..program_len]);

        let mut packed = [0u32; 1024];
        for (index, bytes) in memory.chunks_exact(4).enumerate() {
            packed[index] = (bytes[0] as u32)
                | ((bytes[1] as u32) << 8)
                | ((bytes[2] as u32) << 16)
                | ((bytes[3] as u32) << 24);
        }

        Self {
            block_id: 0,
            i_reg: 0,
            sp: 0,
            delay_timer: 0,
            sound_timer: 0,
            rng_state: 0x1234_5678,
            v: [0; 4],
            stack: [0; 8],
            memory: packed,
        }
    }
}

const RENDER_SHADER: &str = r#"
struct VertexOut {
    @builtin(position) position: vec4f,
    @location(0) uv: vec2f,
};

@vertex
fn vs_main(@builtin(vertex_index) index: u32) -> VertexOut {
    var positions = array<vec2f, 3>(
        vec2f(-1.0, -3.0),
        vec2f(-1.0,  1.0),
        vec2f( 3.0,  1.0),
    );
    var out: VertexOut;
    let p = positions[index];
    out.position = vec4f(p, 0.0, 1.0);
    out.uv = p * vec2f(0.5, -0.5) + vec2f(0.5, 0.5);
    return out;
}

@group(0) @binding(0) var<storage, read> display: array<u32, 2048>;

@fragment
fn fs_main(in: VertexOut) -> @location(0) vec4f {
    let x = min(63u, u32(in.uv.x * 64.0));
    let y = min(31u, u32(in.uv.y * 32.0));
    let on = f32(display[y * 64u + x]);
    return vec4f(on, on, on, 1.0);
}
"#;

pub struct Chip8App;

impl Chip8App {
    pub fn compile_rom_file(rom_path: &Path, output_path: Option<&Path>) -> AppResult<PathBuf> {
        let rom = std::fs::read(rom_path)?;
        let shader = compiler::compile(&rom);
        let stem = rom_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("rom");
        let target = output_path.map(PathBuf::from).unwrap_or_else(|| {
            PathBuf::from("target")
                .join("generated")
                .join(format!("{stem}.compute.wgsl"))
        });

        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(&target, shader)?;
        Ok(target)
    }

    pub fn visualize_rom_file(rom_path: &Path, frames: u32) -> AppResult<PathBuf> {
        let shader_path = Self::compile_rom_file(rom_path, None)?;
        let stem = rom_path
            .file_stem()
            .and_then(|value| value.to_str())
            .unwrap_or("rom");
        let image_path = PathBuf::from("target")
            .join("generated")
            .join(format!("{stem}.ppm"));

        let shader_src = std::fs::read_to_string(&shader_path)?;
        let rom = std::fs::read(rom_path)?;
        let vm = VmState::from_rom(&rom);

        let instance = wgpu::Instance::default();
        let adapter =
            pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions::default()))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("chip8-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            }))?;

        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("compute-shader"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });

        let vm_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vm"),
            contents: bytemuck::bytes_of(&vm),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let display_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("display"),
            contents: bytemuck::cast_slice(&vec![0u32; WIDTH * HEIGHT]),
            usage: wgpu::BufferUsages::STORAGE,
        });
        let keypad_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("keypad"),
            contents: bytemuck::cast_slice(&vec![0u32; 16]),
            usage: wgpu::BufferUsages::STORAGE,
        });

        let compute_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("compute-layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: true },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });

        let compute_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("compute-group"),
            layout: &compute_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: vm_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: display_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: keypad_buffer.as_entire_binding(),
                },
            ],
        });

        let compute_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("compute-pipeline-layout"),
                bind_group_layouts: &[&compute_layout],
                immediate_size: 0,
            });

        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("compute-pipeline"),
            layout: Some(&compute_pipeline_layout),
            module: &compute_shader,
            entry_point: Some("main"),
            compilation_options: wgpu::PipelineCompilationOptions::default(),
            cache: None,
        });

        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("render-shader"),
            source: wgpu::ShaderSource::Wgsl(RENDER_SHADER.into()),
        });

        let render_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("render-layout"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Storage { read_only: true },
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });

        let render_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("render-group"),
            layout: &render_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: display_buffer.as_entire_binding(),
            }],
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("render-pipeline-layout"),
                bind_group_layouts: &[&render_layout],
                immediate_size: 0,
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("render-pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &render_shader,
                entry_point: Some("vs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            fragment: Some(wgpu::FragmentState {
                module: &render_shader,
                entry_point: Some("fs_main"),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                targets: &[Some(wgpu::ColorTargetState {
                    format: wgpu::TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("frame-texture"),
            size: wgpu::Extent3d {
                width: WIDTH as u32,
                height: HEIGHT as u32,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let texture_view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        let readback_size = (WIDTH * HEIGHT * 4) as u64;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("frame-readback"),
            size: readback_size,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });

        let mut encoder =
            device.create_command_encoder(&wgpu::CommandEncoderDescriptor { label: None });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("compute-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&compute_pipeline);
            pass.set_bind_group(0, &compute_group, &[]);
            for _ in 0..frames {
                pass.dispatch_workgroups(1, 1, 1);
            }
        }

        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &texture_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::BLACK),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                timestamp_writes: None,
                occlusion_query_set: None,
                multiview_mask: None,
            });
            pass.set_pipeline(&render_pipeline);
            pass.set_bind_group(0, &render_group, &[]);
            pass.draw(0..3, 0..1);
        }

        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &readback,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some((WIDTH * 4) as u32),
                    rows_per_image: Some(HEIGHT as u32),
                },
            },
            wgpu::Extent3d {
                width: WIDTH as u32,
                height: HEIGHT as u32,
                depth_or_array_layers: 1,
            },
        );

        queue.submit(Some(encoder.finish()));
        let _ = device.poll(wgpu::PollType::wait_indefinitely());

        let slice = readback.slice(..);
        let (tx, rx) = std::sync::mpsc::sync_channel(1);
        slice.map_async(wgpu::MapMode::Read, move |result| {
            tx.send(result).expect("map callback send failed");
        });
        let _ = device.poll(wgpu::PollType::wait_indefinitely());
        rx.recv().expect("map callback recv failed")?;
        let bytes = slice.get_mapped_range().to_vec();
        readback.unmap();

        if let Some(parent) = image_path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let mut file = std::fs::File::create(&image_path)?;
        write!(file, "P6\n{} {}\n255\n", WIDTH, HEIGHT)?;
        for pixel in bytes.chunks_exact(4) {
            file.write_all(&pixel[..3])?;
        }

        Ok(image_path)
    }
}
