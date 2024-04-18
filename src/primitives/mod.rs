pub mod triangle;
pub mod quad;

pub trait Vertex {
    fn layout() -> wgpu::VertexBufferLayout<'static>;
}
