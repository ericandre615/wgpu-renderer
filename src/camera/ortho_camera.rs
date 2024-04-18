use cgmath::*;

// The coordinate system in Wgpu is based on DirectX, and Metal's coordinate systems.
// That means that in normalized device coordinates (opens new window)
// the x axis and y axis are in the range of -1.0 to +1.0,
// and the z axis is 0.0 to +1.0.
// The cgmath crate (as well as most game math crates) is built for OpenGL's coordinate system.
// This matrix will scale and translate our scene from OpenGL's coordinate system to WGPU's.
// We'll define it as follows.
#[rustfmt::skip]
pub const OPENGL_TO_WGPU_MATRIX: Matrix4<f32> = Matrix4::new(
    1.0, 0.0, 0.0, 0.0,
    0.0, 1.0, 0.0, 0.0,
    0.0, 0.0, 0.5, 0.5,
    0.0, 0.0, 0.0, 1.0,
);

// We need this for Rust to store our data correctly for the shaders
#[repr(C)]
// this is so we can store this in a buffer
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct OrthoCameraUniform {
    // we can't use cgmath with bytemuck directly so we'll have
    // to convert the Matrix4 into a 4x4 f32 array
    pub view_position: [f32; 4],
    pub view_projection: [[f32; 4]; 4],
}

impl OrthoCameraUniform {
    pub fn new() -> Self {
        use cgmath::SquareMatrix;

        Self {
            view_position: [0.0; 4],
            view_projection: Matrix4::identity().into(),
        }
    }

    pub fn update_view_projection(&mut self, camera: &OrthoCamera, projection: &OrthoProjection) {
        // self.view_position = camera.position.to_homogeneous().into();
        let position: Point3<f32> = Point3 {
            x: camera.position.x,
            y: camera.position.y,
            z: camera.position.z,
        }.into();
        self.view_position = position.to_homogeneous().into();
        self.view_projection = (projection.calc_matrix() * camera.calc_matrix()).into();
    }
}

pub struct OrthoCameraBuffer {
    pub buffer: wgpu::Buffer,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
}

impl OrthoCameraBuffer {
    pub fn new(device: &wgpu::Device, camera: &OrthoCamera, uniform: &mut OrthoCameraUniform, projection: &OrthoProjection) -> Self {
        uniform.update_view_projection(&camera, &projection);

        let buffer = OrthoCameraBuffer::create_buffer(device, camera, uniform, projection);
        let bind_group_layout = OrthoCameraBuffer::create_bind_group_layout(device);
        let bind_group = OrthoCameraBuffer::create_bind_group(device, &bind_group_layout, &buffer);

        Self {
            buffer,
            bind_group_layout,
            bind_group,
        }
    }

    pub fn create_buffer(
        device: &wgpu::Device,
        camera: &OrthoCamera,
        uniform: &mut OrthoCameraUniform,
        projection: &OrthoProjection,
    ) -> wgpu::Buffer {
        use wgpu::util::DeviceExt;

        uniform.update_view_projection(&camera, &projection);

        let camera_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Ortho Camera Buffer"),
                contents: bytemuck::cast_slice(&[* uniform]), // TODO: not exactly sure about this
                usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            }
        );

        camera_buffer
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
                label: Some("ortho_camera_bind_group_layout"),
            },
        );

        bind_group_layout
    }

    pub fn create_bind_group(
        device: &wgpu::Device,
        bind_group_layout: &wgpu::BindGroupLayout,
        buffer: &wgpu::Buffer
    ) -> wgpu::BindGroup {
        let camera_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }
            ],
            label: Some("ortho_camera_bind_group"),
        });

        camera_bind_group
    }
}

pub struct OrthoProjection {
    width: f32,
    height: f32,
    znear: f32,
    zfar: f32,
}

impl Default for OrthoProjection {
    fn default() -> Self {
        Self {
            width: 0.0,
            height: 0.0,
            znear: -1.0,
            zfar: 1.0,
        }
    }
}

impl OrthoProjection {
    pub fn new(
        width: u32,
        height: u32,
        znear: f32,
        zfar: f32,
    ) -> Self {
        Self {
            width: width as f32,
            height: height as f32,
            znear,
            zfar,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.width = width as f32;
        self.height = height as f32;
    }

    pub fn calc_matrix(&self) -> Matrix4<f32> {
        // OPENGL_TO_WGPU_MATRIX * ortho(0.0, self.width, self.height, 0.0, self.znear, self.zfar)
        // let projection: Matrix4<f32> = [
        //     [2.0 / self.width, 0.0, 0.0, 0.0],
        //     [0.0, -2.0 / self.height, 0.0, 0.0],
        //     [0.0, 0.0, 1.0, 0.0],
        //     [-1.0, 1.0, 0.0, 1.0],
        // ].into();
        let projection = cgmath::ortho(0.0, self.width, self.height, 0.0, -10.0, 100.0);

        OPENGL_TO_WGPU_MATRIX * projection
        // projection
    }
}

pub struct OrthoCamera {
    pub position: Vector3<f32>,
    pub dimensions: [f32; 2],
}

impl OrthoCamera {
    pub fn new<
        V: Into<Vector3<f32>>
    >(
        position: V,
        dimensions: [f32; 2],
    ) -> Self {
        Self {
            position: position.into(),
            dimensions,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.dimensions = [width as f32, height as f32];
    }

    pub fn calc_matrix(&self) -> Matrix4<f32> {
        // think this is the "view" matrix (as opposed to "projection"(ortho) matrix)
        // let eye = (0.0, 0.0, 5.0).into();
        // let target = (0.0, 0.0, 0.0).into();
        // let up = cgmath::Vector3::unit_y();
        // cgmath::Matrix4::look_at_rh(eye, target, up)
        cgmath::Matrix4::from_translation(self.position)
    }
}
