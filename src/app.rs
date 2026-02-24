use crate::constants::{
    DEFAULT_SHADER_FILE, FONT, FONT_START, HEIGHT, PROGRAM_START, TICKS_PER_REDRAW, WIDTH,
};
use bytemuck::{Pod, Zeroable};
use std::fs;
use std::path::Path;
use std::sync::Arc;
use thiserror::Error;
use wgpu::util::DeviceExt;
use winit::application::ApplicationHandler;
use winit::error::EventLoopError;
use winit::event::{ElementState, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::keyboard::{KeyCode, PhysicalKey};
use winit::window::Window;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("I/O error: {0}")]
    Io(#[from] std::io::Error),
    #[error("window event loop error: {0}")]
    EventLoop(#[from] EventLoopError),
    #[error("surface creation error: {0}")]
    CreateSurface(#[from] wgpu::CreateSurfaceError),
    #[error("adapter request error: {0}")]
    RequestAdapter(#[from] wgpu::RequestAdapterError),
    #[error("device request error: {0}")]
    RequestDevice(#[from] wgpu::RequestDeviceError),
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub struct EmulatorState {
    pub block_id: u32,
    pub i_reg: u32,
    pub sp: u32,
    pub delay_timer: u32,
    pub sound_timer: u32,
    pub rng_state: u32,
    pub v: [u32; 4],
    pub rpl: [u32; 2],
    pub stack: [u32; 8],
    pub memory: [u32; 1024],
}

impl EmulatorState {
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
            block_id: PROGRAM_START as u32,
            i_reg: 0,
            sp: 0,
            delay_timer: 0,
            sound_timer: 0,
            rng_state: 0x1234_5678,
            v: [0; 4],
            rpl: [0; 2],
            stack: [0; 8],
            memory: packed,
        }
    }
}

const RENDER_SHADER: &str = include_str!("shaders/fragment.wgsl");

pub struct Chip8App;

struct Renderer {
    _instance: wgpu::Instance,
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    surface_config: wgpu::SurfaceConfiguration,
    compute_pipeline: wgpu::ComputePipeline,
    compute_bind_group: wgpu::BindGroup,
    vm_buffer: wgpu::Buffer,
    vm_readback: wgpu::Buffer,
    display_buffer: wgpu::Buffer,
    display_readback: wgpu::Buffer,
    keypad_buffer: wgpu::Buffer,
    render_pipeline: wgpu::RenderPipeline,
    render_bind_group: wgpu::BindGroup,
}

impl Renderer {
    fn new(window: Arc<Window>, shader_src: &str, vm: &EmulatorState) -> Result<Self, AppError> {
        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor::default());
        let surface = instance.create_surface(window.clone())?;
        let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            compatible_surface: Some(&surface),
            force_fallback_adapter: false,
        }))?;
        let (device, queue) =
            pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
                label: Some("chip8-device"),
                required_features: wgpu::Features::empty(),
                required_limits: wgpu::Limits::default(),
                experimental_features: wgpu::ExperimentalFeatures::disabled(),
                memory_hints: wgpu::MemoryHints::Performance,
                trace: wgpu::Trace::Off,
            }))?;

        let size = window.inner_size();
        let caps = surface.get_capabilities(&adapter);
        let surface_format = caps
            .formats
            .iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(caps.formats[0]);
        let present_mode = if caps.present_modes.contains(&wgpu::PresentMode::Fifo) {
            wgpu::PresentMode::Fifo
        } else {
            caps.present_modes[0]
        };
        let alpha_mode = caps.alpha_modes[0];
        let surface_config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width.max(1),
            height: size.height.max(1),
            present_mode,
            alpha_mode,
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };
        surface.configure(&device, &surface_config);

        let compute_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("compute-shader"),
            source: wgpu::ShaderSource::Wgsl(shader_src.into()),
        });
        let vm_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("vm"),
            contents: bytemuck::bytes_of(vm),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        });
        let vm_readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("vm-readback"),
            size: std::mem::size_of::<EmulatorState>() as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let display_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("display"),
            contents: bytemuck::cast_slice(&vec![0u32; WIDTH * HEIGHT]),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        });
        let display_readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("display-readback"),
            size: (WIDTH * HEIGHT * std::mem::size_of::<u32>()) as u64,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let keypad_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("keypad"),
            contents: bytemuck::cast_slice(&[0u32; 16]),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
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
        let compute_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
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
        let render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
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
                    format: surface_format,
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });

        Ok(Self {
            _instance: instance,
            surface,
            device,
            queue,
            surface_config,
            compute_pipeline,
            compute_bind_group,
            vm_buffer,
            vm_readback,
            display_buffer,
            display_readback,
            keypad_buffer,
            render_pipeline,
            render_bind_group,
        })
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        self.surface_config.width = new_size.width.max(1);
        self.surface_config.height = new_size.height.max(1);
        self.surface.configure(&self.device, &self.surface_config);
    }

    fn render(&mut self, keypad: &[u32; 16]) -> Result<(), wgpu::SurfaceError> {
        self.queue
            .write_buffer(&self.keypad_buffer, 0, bytemuck::cast_slice(keypad));

        let frame = self.surface.get_current_texture()?;
        let view = frame
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());
        let mut encoder = self
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("frame-encoder"),
            });
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("compute-pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.compute_pipeline);
            pass.set_bind_group(0, &self.compute_bind_group, &[]);
            for _ in 0..TICKS_PER_REDRAW {
                pass.dispatch_workgroups(1, 1, 1);
            }
        }
        {
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("render-pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
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
            pass.set_pipeline(&self.render_pipeline);
            pass.set_bind_group(0, &self.render_bind_group, &[]);
            pass.draw(0..3, 0..1);
        }
        encoder.copy_buffer_to_buffer(
            &self.vm_buffer,
            0,
            &self.vm_readback,
            0,
            std::mem::size_of::<EmulatorState>() as u64,
        );
        encoder.copy_buffer_to_buffer(
            &self.display_buffer,
            0,
            &self.display_readback,
            0,
            (WIDTH * HEIGHT * std::mem::size_of::<u32>()) as u64,
        );
        self.queue.submit(Some(encoder.finish()));
        frame.present();
        Ok(())
    }
}

struct App {
    shader_src: String,
    vm: EmulatorState,
    window: Option<Arc<Window>>,
    gpu: Option<Renderer>,
    keypad: [u32; 16],
}

impl ApplicationHandler for App {
    fn resumed(&mut self, event_loop: &ActiveEventLoop) {
        if self.window.is_some() {
            return;
        }
        let window = match event_loop.create_window(
            Window::default_attributes()
                .with_title("chip8-rs")
                .with_inner_size(winit::dpi::PhysicalSize::new(960u32, 480u32))
                .with_resizable(true),
        ) {
            Ok(window) => Arc::new(window),
            Err(error) => {
                eprintln!("failed to create window: {error}");
                event_loop.exit();
                return;
            }
        };
        let gpu = match Renderer::new(window.clone(), &self.shader_src, &self.vm) {
            Ok(gpu) => gpu,
            Err(error) => {
                eprintln!("failed to initialize GPU: {error}");
                event_loop.exit();
                return;
            }
        };
        self.window = Some(window);
        self.gpu = Some(gpu);
    }

    fn window_event(
        &mut self,
        event_loop: &ActiveEventLoop,
        window_id: winit::window::WindowId,
        event: WindowEvent,
    ) {
        let Some(window) = &self.window else {
            return;
        };
        if window.id() != window_id {
            return;
        }
        match event {
            WindowEvent::CloseRequested => event_loop.exit(),
            WindowEvent::Resized(new_size) => {
                if let Some(gpu) = &mut self.gpu {
                    gpu.resize(new_size);
                }
            }
            WindowEvent::RedrawRequested => {
                if let Some(gpu) = &mut self.gpu {
                    match gpu.render(&self.keypad) {
                        Ok(()) => {}
                        Err(wgpu::SurfaceError::Outdated | wgpu::SurfaceError::Lost) => {
                            gpu.surface.configure(&gpu.device, &gpu.surface_config);
                        }
                        Err(wgpu::SurfaceError::OutOfMemory) => event_loop.exit(),
                        Err(wgpu::SurfaceError::Timeout | wgpu::SurfaceError::Other) => {}
                    }
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                if let PhysicalKey::Code(code) = event.physical_key {
                    let value = match event.state {
                        ElementState::Pressed => 1u32,
                        ElementState::Released => 0u32,
                    };
                    if code == KeyCode::Escape && value == 1 {
                        event_loop.exit();
                        return;
                    }
                    if let Some(index) = keycode_to_chip8(code) {
                        self.keypad[index] = value;
                    }
                }
            }
            _ => {}
        }
    }

    fn about_to_wait(&mut self, _event_loop: &ActiveEventLoop) {
        if let Some(window) = &self.window {
            window.request_redraw();
        }
    }
}

impl Chip8App {
    pub fn run(rom_path: &Path) -> Result<(), AppError> {
        let shader_src = fs::read_to_string(DEFAULT_SHADER_FILE)?;
        let rom = std::fs::read(rom_path)?;
        let vm = EmulatorState::from_rom(&rom);

        let event_loop = EventLoop::new()?;
        event_loop.set_control_flow(ControlFlow::Poll);
        let mut app = App {
            shader_src,
            vm,
            window: None,
            gpu: None,
            keypad: [0; 16],
        };
        event_loop.run_app(&mut app)?;

        Ok(())
    }
}

fn keycode_to_chip8(key: KeyCode) -> Option<usize> {
    match key {
        KeyCode::Digit1 => Some(0x1),
        KeyCode::Digit2 => Some(0x2),
        KeyCode::Digit3 => Some(0x3),
        KeyCode::Digit4 => Some(0xC),
        KeyCode::KeyQ => Some(0x4),
        KeyCode::KeyW => Some(0x5),
        KeyCode::KeyE => Some(0x6),
        KeyCode::KeyR => Some(0xD),
        KeyCode::KeyA => Some(0x7),
        KeyCode::KeyS => Some(0x8),
        KeyCode::KeyD => Some(0x9),
        KeyCode::KeyF => Some(0xE),
        KeyCode::KeyZ => Some(0xA),
        KeyCode::KeyX => Some(0x0),
        KeyCode::KeyC => Some(0xB),
        KeyCode::KeyV => Some(0xF),
        _ => None,
    }
}
