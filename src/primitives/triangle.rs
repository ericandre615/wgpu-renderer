type Position = [f32; 3];
type Color = [f32; 3];

use std::ops::Range;
use crate::primitives::Vertex;

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TriangleVertex {
    pub position: Position,
    pub color: Color,
}

impl Vertex for TriangleVertex {
    fn layout() -> wgpu::VertexBufferLayout<'static> {
        wgpu::VertexBufferLayout {
            array_stride: std::mem::size_of::<TriangleVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex, // can also be Instance
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x3,
                },
                wgpu::VertexAttribute {
                    offset: std::mem::size_of::<[f32; 3]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x3,
                }
            ],
        }
    }
}

pub struct Triangle {
    pub vertices: [TriangleVertex; 3],
    pub indices: [u16; 3],
    pub vertex_buffer: wgpu::Buffer,
    pub index_buffer: wgpu::Buffer,
}

impl Triangle {
    pub fn new(vertices: [TriangleVertex; 3], device: &wgpu::Device) -> Self {
        use wgpu::util::DeviceExt;
        let vertex_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Triangle Vertex Buffer"),
                contents: bytemuck::cast_slice(&vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }
        );
        let indices = [
            0, 1, 2,
        ];
        let index_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Triangle Index Buffer"),
                contents: bytemuck::cast_slice(&indices),
                usage: wgpu::BufferUsages::INDEX,
            }
        );

        Self {
            vertices,
            indices,
            vertex_buffer,
            index_buffer,
        }
    }
}

pub trait DrawTriangle<'a> {
    fn draw_triangle(
        &mut self,
        triangle: &'a Triangle,
        camera_bind_group: &'a wgpu::BindGroup,
    );

    fn draw_triangle_instanced(
        &mut self,
        triangle: &'a Triangle,
        instances: Range<u32>,
        camera_bind_group: &'a wgpu::BindGroup,
    );
}

impl<'a, 'b> DrawTriangle<'b> for wgpu::RenderPass<'a>
where
    'b: 'a,
{
    fn draw_triangle(
        &mut self,
        triangle: &'b Triangle,
        camera_bind_group: &'b wgpu::BindGroup,
    ) {
        self.draw_triangle_instanced(triangle, 0..1, camera_bind_group);
    }

    fn draw_triangle_instanced(
        &mut self,
        triangle: &'b Triangle,
        instances: Range<u32>,
        camera_bind_group: &'b wgpu::BindGroup,
    ) {
        let num_indices = triangle.indices.len() as u32;
        self.set_vertex_buffer(0, triangle.vertex_buffer.slice(..));
        self.set_index_buffer(triangle.index_buffer.slice(..), wgpu::IndexFormat::Uint32);
        self.set_bind_group(0, camera_bind_group, &[]);
        self.draw_indexed(0..num_indices, 0, instances);
    }
}
