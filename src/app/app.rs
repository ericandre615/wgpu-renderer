use winit::{
    event::*,
    window::Window,
    dpi::PhysicalPosition,
};

use cgmath::prelude::*;

use wgpu::util::DeviceExt;

use crate::render::create_render_pipeline;
use crate::texture::Texture;
use crate::camera::{
    Camera,
    CameraUniform,
    CameraBuffer,
    CameraController,
    Projection,
};
use crate::camera::{
    OrthoCamera,
    OrthoCameraUniform,
    OrthoCameraBuffer,
    OrthoProjection,
};

use crate::primitives::{
    Vertex,
    triangle::{TriangleVertex, Triangle},
    quad::{QuadVertex, Quad, QuadOptions},
};
use crate::instance::{Instance, InstanceRaw, InstanceBuffer};
use crate::model::{ModelVertex, Model};
use crate::light::Light;
use crate::resources;

const INDICES: &[u16] = &[
    0, 1, 4,
    1, 2, 4,
    2, 3, 4,
];

const SPACE_BETWEEN: f32 = 3.0;

pub struct Camera2D {
    camera: OrthoCamera,
    uniform: OrthoCameraUniform,
    buffer: OrthoCameraBuffer,
    projection: OrthoProjection,
}

pub fn create_instances(amount: u32) -> Vec<Instance> {
    // let displacement: cgmath::Vector3<f32> = cgmath::Vector3::new(amount as f32 * 0.5, 0.0, amount as f32 * 0.5);

    let instances = (0..amount).flat_map(|z| {
        (0..amount).map(move |x| {
            let x = SPACE_BETWEEN * (x as f32 - amount as f32 / 2.0);
            let z = SPACE_BETWEEN * (z as f32 - amount as f32 / 2.0);

            let position = cgmath::Vector3 {
                x: x as f32,
                y: 0.0,
                z: z as f32,
            };

            let rotation = if position.is_zero() {
                // this is needed so an object at (0, 0, 0) won't get scaled to zero
                // as Quaternions can effect scale if they're not created correctly
                cgmath::Quaternion::from_axis_angle(cgmath::Vector3::unit_z(), cgmath::Deg(0.0))
            } else {
                cgmath::Quaternion::from_axis_angle(position.normalize(), cgmath::Deg(45.0))
            };

            Instance {
                position,
                rotation,
            }
        })
    }).collect::<Vec<_>>();

    instances
}

pub struct App {
    pub surface: wgpu::Surface,
    pub device: wgpu::Device,
    pub queue: wgpu::Queue,
    pub config: wgpu::SurfaceConfiguration,
    pub size: winit::dpi::PhysicalSize<u32>,
    // The window must be declared after the surface so
    // it gets dropped after it as the surface contains
    // unsafe references to the window's resources.
    pub window: Rc<Window>,

    pub render_pipeline: wgpu::RenderPipeline,
    pub render_pipeline_2d: wgpu::RenderPipeline,
    pub light_render_pipeline: wgpu::RenderPipeline,
    // pub vertex_buffer: wgpu::Buffer,
    // pub index_buffer: wgpu::Buffer,
    // pub diffuse_bind_group: wgpu::BindGroup,
    // pub diffuse_texture: Texture,
    pub camera: Camera,
    pub camera_uniform: CameraUniform,
    pub camera_buffer: CameraBuffer,
    pub camera_controller: CameraController,
    pub projection: Projection,

    pub ortho_camera: Camera2D,

    pub instances: Vec<Instance>,
    pub instance_buffer: InstanceBuffer,
    pub depth_texture: Texture,
    pub obj_model: Model,

    pub light: Light,
    pub light_model: Model,

    pub quad_model: Quad,
    // pub quad_model_too: Quad,

    pub mouse_pressed: bool,
}

use std::rc::Rc;

impl App {
    // Creating some of the wgpu types requires async code
    pub async fn new(window: Rc<Window>) -> Self {
        let size = window.inner_size();

        use std::ops::Deref;
        let window_ref = window.clone();
        let window = window.deref();

        // The instance is a handle to our GPU
        // Backends::all => Vulkan + Metal + DX12 + Browser WebGPU
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::all(),
            dx12_shader_compiler: Default::default(),
        });

        // # Safety
        //
        // The surface needs to live as long as the window that created it.
        // State owns the wndow so this should be safe
        let surface = unsafe { instance.create_surface(&window) }.unwrap();

        // adapter is a handle to the actual graphics card
        // request_adapter can return None if wgpu can't find an adapter with required permissions
        // enumerate_adapters will iterate to check for an aapter you need, but does not work on
        // wasm
        let adapter = instance.request_adapter(
            &wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            },
        ).await.unwrap();

        let (device, queue) = adapter.request_device(
            &wgpu::DeviceDescriptor {
                features: wgpu::Features::empty(),
                // WebGL does not support all wgpu features
                // disable them when building for the web
                limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits {
                        max_texture_dimension_2d: 16384, // this is the max size of texture for WebGL2
                        ..wgpu::Limits::downlevel_webgl2_defaults()
                    }
                } else {
                    wgpu::Limits::default()
                },
                label: None,
            },
            None, // Trace path
        ).await.unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        // This only supports Shader code that assumes an sRGB surface texture. Using a different
        // one will result all the colors coming out darker. If you want to support non
        // sRGB surfaces, you'll need to account for that when drawing to the frame.
        let surface_format = surface_caps.formats.iter()
            .copied()
            .find(|f| f.is_srgb())
            .unwrap_or(surface_caps.formats[0]);
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
        };

        surface.configure(&device, &config);

        // let diffuse_bytes = include_bytes!("../assets/mario-sprite.png");
        // let diffuse_texture = Texture::from_bytes(&device, &queue, diffuse_bytes, "mario-sprite.png").unwrap();

        let texture_bind_group_layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            view_dimension: wgpu::TextureViewDimension::D2,
                            sample_type: wgpu::TextureSampleType::Float {
                                filterable: true,
                            },
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 1,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        // this should match the filterable field of the corresponding Texture
                        // Entry above
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                    // normal map
                    wgpu::BindGroupLayoutEntry {
                        binding: 2,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Texture {
                            multisampled: false,
                            sample_type: wgpu::TextureSampleType::Float { filterable: true },
                            view_dimension: wgpu::TextureViewDimension::D2,
                        },
                        count: None,
                    },
                    wgpu::BindGroupLayoutEntry {
                        binding: 3,
                        visibility: wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                        count: None,
                    },
                ],
                label: Some("texture_bind_group_layout"),
            }
        );

        // let diffuse_bind_group = device.create_bind_group(
        //     &wgpu::BindGroupDescriptor {
        //         layout: &texture_bind_group_layout,
        //         entries: &[
        //             wgpu::BindGroupEntry {
        //                 binding: 0,
        //                 resource: wgpu::BindingResource::TextureView(&diffuse_texture.view),
        //             },
        //             wgpu::BindGroupEntry {
        //                 binding: 1,
        //                 resource: wgpu::BindingResource::Sampler(&diffuse_texture.sampler),
        //             }
        //         ],
        //         label: Some("diffuse_bind_group"),
        //     }
        // );

        // let camera = Camera {
        //     // posiition the camera one unit up and 2 units back
        //     // +z is out of the screen
        //     eye: (0.0, 1.0, 2.0).into(),
        //     // we have it look at the origin
        //     target: (0.0, 0.0, 0.0).into(),
        //     // which way is up?
        //     up: cgmath::Vector3::unit_y(),
        //     aspect: config.width as f32 / config.height as f32,
        //     fovy: 45.0,
        //     znear: 0.1,
        //     zfar: 100.0,
        // };
        let camera = Camera::new((0.0, 5.0, 10.0), cgmath::Deg(-90.0), cgmath::Deg(-20.0));
        let projection = Projection::new(config.width, config.height, cgmath::Deg(45.0), 0.1, 100.0);

        let mut camera_uniform = CameraUniform::new();
        camera_uniform.update_view_projection(&camera, &projection);
        let camera_buffer = CameraBuffer::new(&device, &camera, &mut camera_uniform, &projection);
        // let camera_bind_group_layout = camera_uniform.create_bind_group_layout(&device);
        let camera_controller = CameraController::new(4.0, 0.4);

        let ortho_cam = OrthoCamera::new((0.0, 0.0, 0.0), [config.width as f32, config.height as f32]);
        let mut ortho_uniform = OrthoCameraUniform::new();
        let ortho_projection = OrthoProjection::new(config.width, config.height, -1.0, 1.0);
        let ortho_buffer =  OrthoCameraBuffer::new(&device, &ortho_cam, &mut ortho_uniform, &ortho_projection);
        let ortho_camera = Camera2D {
            camera: ortho_cam,
            uniform: ortho_uniform,
            buffer: ortho_buffer,
            projection: ortho_projection,
        };

        let instances = create_instances(1); // (10);
        let instance_buffer = InstanceBuffer::new(&device, &instances);

        let depth_texture = Texture::create_depth_texture(&device, &config, "depth_texture");

        let light = Light::new(&device, [2.0, 2.0, 2.0], [1.0, 1.0, 1.0]);

        // let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        //     label: Some("Shader"),
        //     source: wgpu::ShaderSource::Wgsl(
        //         include_str!("../shaders/shader.wgsl").into()
        //     )
        // });

        let render_pipline_layout = device.create_pipeline_layout(
            &wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[
                    &texture_bind_group_layout,
                    &camera_buffer.bind_group_layout,
                    &light.bind_group_layout,
                ],
                push_constant_ranges: &[],
            }
        );

        let render_pipeline = {
            let shader = wgpu::ShaderModuleDescriptor {
                label: Some("Normal Shader"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("../shaders/shader.wgsl").into()
                ),
            };

            create_render_pipeline(
                &device,
                &render_pipline_layout,
                config.format,
                Some(Texture::DEPTH_FORMAT),
                &[ModelVertex::layout(), InstanceRaw::layout()],
                shader,
                None,
            )
        };

        let light_render_pipeline = {
            let light_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Light Pipeline Layout"),
                bind_group_layouts: &[&camera_buffer.bind_group_layout, &light.bind_group_layout],
                push_constant_ranges: &[],
            });

            let shader = wgpu::ShaderModuleDescriptor {
                label: Some("Light Shader"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("../shaders/light.wgsl").into()
                ),
            };

            create_render_pipeline(
                &device,
                &light_pipeline_layout,
                config.format,
                Some(Texture::DEPTH_FORMAT),
                &[ModelVertex::layout()],
                shader,
                None,
            )
        };

        // TODO: figure out translating between screen (top/left) and (width/height of objects)
        // dividing quad width/height by window width.height is closer? but now only drawing a
        // triangle instead of a quad
        let quad_model = Quad::new(&device, &config, QuadOptions {
            position: [0.0, 0.0],
            color: (10, 207, 131, 0.5),
            dimensions: (400.0, 200.0),
        });
        let render_pipeline_2d = {
            let render_pipeline_2d_layout = device.create_pipeline_layout(
                &wgpu::PipelineLayoutDescriptor {
                    label: Some("2d Render Pipeline Lauout"),
                    bind_group_layouts: &[
                        // need camera_2d_buffer.bind_group_layout,
                        &ortho_camera.buffer.bind_group_layout,
                        // need triangle.bing_group_layout (or rectangle? whatever we need here)
                        &quad_model.uniform_buffer.bind_group_layout,
                    ],
                    push_constant_ranges: &[],
                }
            );

            let shader_2d = wgpu::ShaderModuleDescriptor {
                label: Some("2D Shader"),
                source: wgpu::ShaderSource::Wgsl(
                    include_str!("../shaders/quad.wgsl").into()
                ),
            };

            create_render_pipeline(
                &device,
                &render_pipeline_2d_layout,
                config.format,
                Some(Texture::DEPTH_FORMAT),
                &[QuadVertex::layout()],
                shader_2d,
                // Some(wgpu::BlendState {
                //     color: wgpu::BlendComponent {
                //         src_factor: wgpu::BlendFactor::SrcAlpha,
                //         dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                //         operation: wgpu::BlendOperation::Add,
                //     },
                //     alpha: wgpu::BlendComponent::OVER,
                // }),
                Some(wgpu::BlendState::ALPHA_BLENDING),
            )
        };

        // let render_pipeline = device.create_render_pipeline(
        //     &wgpu::RenderPipelineDescriptor {
        //         label: Some("Render Pipeline"),
        //         layout: Some(&render_pipline_layout),
        //         vertex: wgpu::VertexState {
        //             module: &shader,
        //             entry_point: "vs_main", // 1
        //             buffers: &[ModelVertex::layout(), InstanceRaw::layout()], // 2
        //         },
        //         fragment: Some(wgpu::FragmentState { // 3
        //             module: &shader,
        //             entry_point: "fs_main",
        //             targets: &[Some(wgpu::ColorTargetState { // 4
        //                 format: config.format,
        //                 blend: Some(wgpu::BlendState::REPLACE),
        //                 write_mask: wgpu::ColorWrites::ALL,
        //             })],
        //         }),
        //         primitive: wgpu::PrimitiveState {
        //             topology: wgpu::PrimitiveTopology::TriangleList,
        //             strip_index_format: None,
        //             front_face: wgpu::FrontFace::Ccw,
        //             cull_mode: Some(wgpu::Face::Back),
        //             polygon_mode: wgpu::PolygonMode::Fill,
        //             unclipped_depth: false,
        //             conservative: false,
        //         },
        //         depth_stencil: Some(
        //             wgpu::DepthStencilState {
        //                 format: Texture::DEPTH_FORMAT,
        //                 depth_write_enabled: true,
        //                 depth_compare: wgpu::CompareFunction::Less,
        //                 stencil: wgpu::StencilState::default(),
        //                 bias: wgpu::DepthBiasState::default(),
        //             }
        //         ),
        //         multisample: wgpu::MultisampleState {
        //             count: 1,
        //             mask: !0,
        //             alpha_to_coverage_enabled: false,
        //         },
        //         multiview: None,
        //     }
        // );

        let obj_model = resources::load_model(
            // "meshes/cube/cube.obj",
            // "meshes/monkey/lp-monkey.obj",
            // "meshes/monkey/monkey-rev-c.obj",
            "meshes/greg/greg-applied.obj",
            &device,
            &queue,
            &texture_bind_group_layout
        )
        .await
        .unwrap();

        let light_model = resources::load_model(
            "meshes/light/light-object.obj",
            &device,
            &queue,
            &texture_bind_group_layout,
        )
        .await
        .unwrap();

        // let triangle_model = Triangle::new([
        //     TriangleVertex { position: [0.0, 0.5, 0.0], color: [1.0, 0.0, 0.0] },
        //     TriangleVertex { position: [-0.5, -0.5, 0.0], color: [0.0, 1.0, 0.0] },
        //     TriangleVertex { position: [0.5, -0.5, 0.0], color: [0.0, 0.0, 1.0] },
        // ], &device);

        // let vertex_buffer = device.create_buffer_init(
        //     &wgpu::util::BufferInitDescriptor {
        //         label: Some("Vertex Buffer"),
        //         contents: bytemuck::cast_slice(VERTICES),
        //         usage: wgpu::BufferUsages::VERTEX,
        //     }
        // );
        //
        // let index_buffer = device.create_buffer_init(
        //     &wgpu::util::BufferInitDescriptor {
        //         label: Some("Index Buffer"),
        //         contents: bytemuck::cast_slice(INDICES),
        //         usage: wgpu::BufferUsages::INDEX,
        //     }
        // );
        //
        Self {
            window: window_ref,
            surface,
            device,
            queue,
            config,
            // size should not be 0 as that can lead to app crashes
            size,
            render_pipeline,
            light_render_pipeline,
            render_pipeline_2d,
            // vertex_buffer,
            // index_buffer,
            // diffuse_bind_group,
            // diffuse_texture,
            camera,
            camera_uniform,
            camera_buffer,
            camera_controller,
            projection,

            ortho_camera,

            instances,
            instance_buffer,

            depth_texture,
            obj_model,

            light,
            light_model,

            // triangle_model,
            quad_model,

            mouse_pressed: false,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        // if we want to resize in our app we have to reconfigure the surface
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            self.projection.resize(new_size.width, new_size.height);
            self.ortho_camera.projection.resize(new_size.width, new_size.height);
            self.ortho_camera.camera.resize(new_size.width, new_size.height);
            // update depth_texture after config
            // otherwise, the app will crash because depth_texture will be a different size from
            // the surface
            self.depth_texture = Texture::create_depth_texture(&self.device, &self.config, "depth_texture");
        }
    }

    pub fn input(&mut self, event: &WindowEvent) -> bool {
        // TODO: just returning false for now since there are no events we want to capture
        // false
        // self.camera_controller.process_events(event)
        match event {
            WindowEvent::KeyboardInput {
                input: KeyboardInput {
                    virtual_keycode: Some(key),
                    state,
                    ..
                },
                ..
            } => {
                let [lx, ly, lz] = self.light.uniform.position;
                let _light_key = match key {
                    VirtualKeyCode::B => {
                        self.light.update_position([lx + -1.0, ly, lz]);
                        true
                    },
                    VirtualKeyCode::M => {
                        self.light.update_position([lx + 1.0, ly, lz]);
                        true
                    },
                    VirtualKeyCode::H => {
                        self.light.update_position([lx, ly + 1.0, lz]);
                        true
                    },
                    VirtualKeyCode::N => {
                        self.light.update_position([lx, ly - 1.0, lz]);
                        true
                    },
                    VirtualKeyCode::J => {
                        self.light.update_position([lx, ly, lz - 1.0]);
                        true
                    },
                    VirtualKeyCode::K => {
                        self.light.update_position([lx, ly, lz + 1.0]);
                        true
                    },
                    _ => false,
                };

                self.camera_controller.process_keyboard(*key, *state)
            },
            WindowEvent::MouseWheel { delta, .. } => {
                self.camera_controller.process_scroll(delta);
                true
            },
            WindowEvent::MouseInput {
                button: MouseButton::Left,
                state,
                ..
            } => {
                self.mouse_pressed = *state == ElementState::Pressed;
                true
            },
            _ => false,
        }
    }

    pub fn update(&mut self, dt: instant::Duration) {
        // camera
        self.camera_controller.update_camera(&mut self.camera, dt);
        self.camera_uniform.update_view_projection(&self.camera, &self.projection);
        self.queue.write_buffer(&self.camera_buffer.buffer, 0, bytemuck::cast_slice(&[self.camera_uniform]));

        self.ortho_camera.uniform.update_view_projection(&self.ortho_camera.camera, &self.ortho_camera.projection);
        self.queue.write_buffer(&self.ortho_camera.buffer.buffer, 0, bytemuck::cast_slice(&[self.ortho_camera.uniform]));

        self.quad_model.uniform.update_model_from_position(self.quad_model.options.position);
        self.queue.write_buffer(&self.quad_model.uniform_buffer.buffer, 0, bytemuck::cast_slice(&[self.quad_model.uniform]));
        // light
        // let prev_position: cgmath::Vector3<_> = self.light.uniform.position.into();

        // Animated light effect
        // self.light.uniform.position = (
        //     cgmath::Quaternion::from_axis_angle((0.0, 1.0, 0.0).into(), cgmath::Deg(60.0 * dt.as_secs_f32()))
        //     * prev_position
        // ).into();

        self.queue.write_buffer(&self.light.buffer, 0, bytemuck::cast_slice(&[self.light.uniform]));
    }

    pub fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output.texture.create_view(&wgpu::TextureViewDescriptor::default());

        // We also need to create a CommandEncoder to create the actual commands to send to the gpu.
        // Most modern graphics frameworks expect commands to be stored in a command buffer before being sent to the gpu.
        // The encoder builds a command buffer that we can then send to the gpu.
        let mut encoder = self.device.create_command_encoder(
            &wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            }
        );

        // extra block here is because bgin_render_pass needs a mut ref of encoder
        // but `encoder.finish()` can not be called until we release the mut borrow.
        // an alternative approach would be to use `drop(render_pass)` before calling
        // `encoder.finish()`
        {
            let mut render_pass = encoder.begin_render_pass(
                &wgpu::RenderPassDescriptor {
                    label: Some("Render Pass"),
                    color_attachments: &[
                        // this is what @location(0) in fragment shader targets
                        Some(wgpu::RenderPassColorAttachment {
                            view: &view,
                            resolve_target: None,
                            ops: wgpu::Operations {
                                load: wgpu::LoadOp::Clear(wgpu::Color {
                                    r: 0.1,
                                    g: 0.2,
                                    b: 0.3,
                                    a: 1.0,
                                }),
                                store: true,
                            },
                        })
                    ],
                    depth_stencil_attachment: Some(
                        wgpu::RenderPassDepthStencilAttachment {
                            view: &self.depth_texture.view,
                            depth_ops: Some(wgpu::Operations {
                                load: wgpu::LoadOp::Clear(1.0),
                                store: true,
                            }),
                            stencil_ops: None,
                        }
                    ),
                }
            );


            // render_pass.set_bind_group(0, &self.diffuse_bind_group, &[]);
            // render_pass.set_bind_group(1, &self.camera_buffer.bind_group, &[]);

            // render_pass.set_vertex_buffer(0, self.vertex_buffer.slice(..));
            // Make sure if you add new instances to the Vec,
            // that you recreate the instance_buffer and as well as camera_bind_group, otherwise your new instances won't show up correctly.
            render_pass.set_vertex_buffer(1, self.instance_buffer.buffer.slice(..));

            // render_pass.set_index_buffer(self.index_buffer.slice(..), wgpu::IndexFormat::Uint16);

            // render_pass.draw_indexed(0..INDICES.len() as u32, 0, 0..1);
            // render_pass.draw_indexed(0..INDICES.len() as u32, 0, 0..self.instances.len() as _);
            // let mesh = &self.obj_model.meshes[0];
            // let material = &self.obj_model.materials[mesh.material];

            use crate::light::DrawLight;
            render_pass.set_pipeline(&self.light_render_pipeline);
            render_pass.draw_light_model(
                // &self.obj_model,
                &self.light_model,
                &self.camera_buffer.bind_group,
                &self.light.bind_group,
            );

            use crate::model::DrawModel;
            // render_pass.draw_mesh_instanced(
            //     mesh,
            //     material,
            //     0..self.instances.len() as u32,
            //     &self.camera_buffer.bind_group
            // );
            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.draw_model_instanced(
                &self.obj_model,
                0..self.instances.len() as u32,
                &self.camera_buffer.bind_group,
                &self.light.bind_group,
            );

            // use crate::primitives::triangle::DrawTriangle;
            //
            // render_pass.set_pipeline(&self.render_pipeline_2d);
            // render_pass.draw_triangle(
            //     &self.triangle_model,
            //     &self.camera_buffer.bind_group,
            // );
            use crate::primitives::quad::DrawQuad;

            render_pass.set_pipeline(&self.render_pipeline_2d);
            render_pass.draw_quad(
                &self.quad_model,
                &self.ortho_camera.buffer.bind_group,
            );

            // println!("OrthoView {:?}", self.ortho_camera.uniform.view_position);
            // println!("OrthoProjection {:?}", self.ortho_camera.uniform.view_projection);
            // println!("QuadModel {:?}", self.quad_model.model());
        }

        // submit will accept anything that implements IntoIter
        // these lines tell wgpu to finish the command buffer and submit it to the gpu's render
        // queue
        self.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

