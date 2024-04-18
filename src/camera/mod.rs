pub mod camera;
pub mod controller;
pub mod ortho_camera;

pub use camera::{Camera, CameraUniform, CameraBuffer, Projection};
pub use controller::CameraController;

pub use ortho_camera::{OrthoCamera, OrthoCameraUniform, OrthoCameraBuffer, OrthoProjection};

pub type OrthoMatrix = [[f32; 4]; 4];

pub fn get_ortho_matrix(width: f32, height: f32) -> OrthoMatrix {
    [
        [2.0 / width, 0.0,           0.0, 0.0],
        [0.0,         -2.0 / height, 0.0, 0.0],
        [0.0,         0.0,           1.0, 0.0],
        [-1.0,        1.0,           0.0, 1.0],
    ]
}

pub fn get_ortho_projection_matrix(width: f32, height: f32) -> cgmath::Matrix4<f32> {
    cgmath::ortho(0.0, width, height, 0.0, 1.0, -1.0)
}
