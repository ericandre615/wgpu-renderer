type VertexPosition = [f32; 3];
type VertexColor = [f32; 4];
type QuadDimensions = (f32 /* width */, f32 /* height */);
type QuadPosition = [f32; 2];
type ColorRGBA = (u32, u32, u32, f32);

use std::ops::Range;
use crate::primitives::Vertex;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct QuadVertex {
    pub position: VertexPosition,
    pub color: VertexColor,
}

impl Vertex for QuadVertex {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<QuadVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex, // can also be Instance
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<VertexPosition>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                }
            ],
        }
    }
}

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct QuadUniform {
    pub model: [[f32; 4]; 4],
}

impl QuadUniform {
    pub fn new() -> Self {
        use cgmath::SquareMatrix;

        Self {
            model: cgmath::Matrix4::identity().into(),
        }
    }

    pub fn update_model(&mut self, quad: &Quad) {
        self.model = quad.model().into();
    }

    pub fn update_model_from_position(&mut self, position: [f32; 2]) {
        let [x,y] = position;
        let model_position = cgmath::Vector3::new(x, y, 0.0);
        let model = cgmath::Matrix4::from_translation(model_position);

        self.model = model.into();
    }
}

pub struct QuadUniformBuffer {
    pub buffer: wgpu::Buffer,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
}

impl QuadUniformBuffer {
    pub fn new(device: &wgpu::Device, uniform: &mut QuadUniform, position: [f32; 2]) -> Self {
        use cgmath::SquareMatrix;

        uniform.update_model_from_position(position);

        let buffer = QuadUniformBuffer::create_buffer(device, uniform, position);
        let bind_group_layout = QuadUniformBuffer::create_bind_group_layout(device);
        let bind_group = QuadUniformBuffer::create_bind_group(device, &bind_group_layout, &buffer);

        Self {
            buffer,
            bind_group_layout,
            bind_group,
        }
    }

    pub fn create_buffer(
        device: &wgpu::Device,
        uniform: &mut QuadUniform,
        position: [f32; 2],
    ) -> wgpu::Buffer {
        use wgpu::util::DeviceExt;

        uniform.update_model_from_position(position);

        let uniform_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Camera Buffer"),
                contents: bytemuck::cast_slice(&[* uniform]), // TODO: not exactly sure about this
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );

        uniform_buffer
    }

    pub fn create_bind_group_layout(device: &wgpu::Device) -> wgpu::BindGroupLayout {
        let bind_group_layout = device.create_bind_group_layout(
            &wgpu::BindGroupLayoutDescriptor {
                entries: &[
                    wgpu::BindGroupLayoutEntry {
                        binding: 0,
                        visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                        ty: wgpu::BindingType::Buffer {
                            ty: wgpu::BufferBindingType::Uniform,
                            has_dynamic_offset: false,
                            min_binding_size: None,
                        },
                        count: None,
                    },
                ],
                label: Some("quad_bind_group_layout"),
            },
        );

        bind_group_layout
    }

    pub fn create_bind_group(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        buffer: &wgpu::Buffer
    ) -> wgpu::BindGroup {
        let quad_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }
            ],
            label: Some("quad_bind_group"),
        });

        quad_bind_group
    }
}

pub struct QuadOptions {
    pub position: QuadPosition,
    pub color: ColorRGBA,
    pub dimensions: QuadDimensions,
}

impl Default for QuadOptions {
    fn default() -> Self {
        Self {
            position: [0.0, 0.0],
            color: (252, 3, 223, 1.0),
            dimensions: (40.0, 40.0),
        }
    }
}

pub struct QuadTransform {
    pub translation: cgmath::Matrix4<f32>,
    pub rotation: cgmath::Matrix4<f32>,
    pub scale: cgmath::Matrix4<f32>,
}

impl Default for QuadTransform {
    fn default() -> QuadTransform {
        let axis = cgmath::Vector3::new(0.0, 1.0, 0.0);
        let angle = cgmath::Deg(0.0);
        let translation = cgmath::Matrix4::from_translation([0.0, 0.0, 0.0].into());
        let rotation = cgmath::Matrix4::from_axis_angle(axis, angle);
        let scale = cgmath::Matrix4::from_scale(1.0);
        QuadTransform {
            translation,
            rotation,
            scale,
        }
    }
}

impl QuadTransform {
}

pub struct Quad {
    pub vertices: [QuadVertex; 4],
    pub indices: [u16; 6],
    pub options: QuadOptions,
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
    pub uniform: QuadUniform,
    pub uniform_buffer: QuadUniformBuffer,
    pub transform: QuadTransform,
}

struct QuadVertexPositions {
    pub x: f32,
    pub y: f32,
    pub x2: f32,
    pub y2: f32,
}

fn calculate_vertex_positions(config: &wgpu::SurfaceConfiguration, quad: &QuadOptions) -> QuadVertexPositions {
    let QuadOptions { position, dimensions, color: _ } = quad;
    let (pos_x, pos_y) = (position[0], position[1]);
    let width = dimensions.0 / config.width as f32;
    let height = dimensions.1 / config.height as f32;

    // let x = pos_x / width * 2.0 - 1.0;
    // let y = pos_y / height * 2.0 - 1.0;
    let x_center = width / 2.0;
    let y_center = height / 2.0;
    let x_shifted = pos_x - x_center;
    let y_shifted = pos_y - y_center;
    // let x = -(pos_x / width);
    // let y = -(pos_y / height);
    let x = x_shifted / (width / 2.0);
    let y = y_shifted / (height / 2.0);

    let x2 = x + width;
    let y2 = y + height;

    QuadVertexPositions {
        x: x2,
        y: y2,
        x2: x,
        y2: y,
    }
}

impl Quad {
    pub fn new(device: &wgpu::Device, config: &wgpu::SurfaceConfiguration, options: QuadOptions) -> Self {
        use wgpu::util::DeviceExt;
        let QuadOptions { position, color, dimensions: _ } = options;
        let quad_color: VertexColor = [
            color.0 as f32 / 255.0,
            color.1 as f32 / 255.0,
            color.2 as f32 / 255.0,
            color.3
        ];
        let QuadVertexPositions {
            x, y, x2, y2
        } = calculate_vertex_positions(&config, &options);

        let vertices = [
            QuadVertex { position: [x, y, 0.0], color: quad_color },
            QuadVertex { position: [x2, y, 0.0], color: quad_color },
            QuadVertex { position: [x, y2, 0.0], color: quad_color },
            QuadVertex { position: [x2, y2, 0.0], color: quad_color },
        ];
        let transform = QuadTransform {
            translation: cgmath::Matrix4::from_translation([x, y, 0.0].into()),
            ..Default::default()
        };
        // let vertices = [
        //     QuadVertex { position: [(x - half_width) / 100.0, (y - half_height) / 100.0, 0.0], color: quad_color },
        //     QuadVertex { position: [(x + half_width) / 100.0, (y - half_height) / 100.0, 0.0], color: quad_color },
        //     QuadVertex { position: [(x - half_width) / 100.0, (y + half_height) / 100.0, 0.0], color: quad_color },
        //     QuadVertex { position: [(x + half_width) / 100.0, (y + half_height) / 100.0, 0.0], color: quad_color },
        // ];
        let vertex_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Quad Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }
        );
        // let indices = [
        //     0, 1, 2,
        //     1, 2, 3,
        // ];
        println!("QUADVERTEX {:?}", vertices);
        let indices = [
            0, 1, 2,
            2, 1, 3,
        ];
        let index_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Quad Index Buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            }
        );
        let mut uniform = QuadUniform::new();
        let uniform_buffer = QuadUniformBuffer::new(device, &mut uniform, position);

        uniform.update_model_from_position(position);

        Self {
            vertices,
            indices,
            options,
            vertex_buffer,
            index_buffer,
            uniform,
            uniform_buffer,
            transform,
        }
    }

    pub fn model_from_position(&self, position: [f32; 2]) -> cgmath::Matrix4<f32> {
        let [x,y] = position;
        let model_position = cgmath::Vector3::new(x, y, 0.0);
        let model = cgmath::Matrix4::from_translation(model_position);

        model
    }

    pub fn model(&self) -> cgmath::Matrix4<f32> {
        let [x,y] = self.options.position;
        let position = cgmath::Vector3::new(x, y, 0.0);
        let model = cgmath::Matrix4::from_translation(position);

        model
    }
}

pub trait DrawQuad<'a> {
    fn draw_quad(
        &mut self,
        quad: &'a Quad,
        camera_bind_group: &'a wgpu::BindGroup,
    );

    fn draw_quad_instanced(
        &mut self,
        quad: &'a Quad,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
    );
}

impl<'a, 'b> DrawQuad<'b> for wgpu::RenderPass<'a>
where
    'b: 'a,
{
    fn draw_quad(
        &mut self,
        quad: &'b Quad,
        camera_bind_group: &'b wgpu::BindGroup,
    ) {
        self.draw_quad_instanced(quad, 0..1, camera_bind_group);
    }

    fn draw_quad_instanced(
        &mut self,
        quad: &'b Quad,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
    ) {
        let num_indices = quad.indices.len() as u32;
        self.set_vertex_buffer(0, quad.vertex_buffer.slice(..));
        self.set_index_buffer(quad.index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        self.set_bind_group(0, camera_bind_group, &[]);
        self.draw_indexed(0..num_indices, 0, instances);
    }
}
