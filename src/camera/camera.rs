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
pub struct CameraUniform {
    // we can't use cgmath with bytemuck directly so we'll have
    // to convert the Matrix4 into a 4x4 f32 array
    pub view_position: [f32; 4],
    pub view_projection: [[f32; 4]; 4],
}

impl CameraUniform {
    pub fn new() -> Self {
        use cgmath::SquareMatrix;

        Self {
            view_position: [0.0; 4],
            view_projection: Matrix4::identity().into(),
        }
    }

    pub fn update_view_projection(&mut self, camera: &Camera, projection: &Projection) {
        // self.view_position = camera.eye.to_homogeneous().into();
        // self.view_projection = (OPENGL_TO_WGPU_MATRIX * camera.build_view_projection_matrix()).into();
        self.view_position = camera.position.to_homogeneous().into();
        self.view_projection = (projection.calc_matrix() * camera.calc_matrix()).into();
    }

    // pub fn create_bind_group_layout(&mut self, device: &wgpu::Device) -> wgpu::BindGroupLayout {
    //     let bind_group_layout = device.create_bind_group_layout(
    //         &wgpu::BindGroupLayoutDescriptor {
    //             entries: &[
    //                 wgpu::BindGroupLayoutEntry {
    //                     binding: 0,
    //                     visibility: wgpu::ShaderStages::VERTEX,
    //                     ty: wgpu::BindingType::Buffer {
    //                         ty: wgpu::BufferBindingType::Uniform,
    //                         has_dynamic_offset: false,
    //                         min_binding_size: None,
    //                     },
    //                     count: None,
    //                 },
    //             ],
    //             label: Some("camera_bind_group_layout"),
    //         },
    //     );
    //
    //     bind_group_layout
    // }
}

pub struct CameraBuffer {
    pub buffer: wgpu::Buffer,
    pub bind_group_layout: wgpu::BindGroupLayout,
    pub bind_group: wgpu::BindGroup,
}

impl CameraBuffer {
    pub fn new(device: &wgpu::Device, camera: &Camera, uniform: &mut CameraUniform, projection: &Projection) -> Self {
        uniform.update_view_projection(&camera, &projection);

        let buffer = CameraBuffer::create_buffer(device, camera, uniform, projection);
        let bind_group_layout = CameraBuffer::create_bind_group_layout(device);
        let bind_group = CameraBuffer::create_bind_group(device, &bind_group_layout, &buffer);

        Self {
            buffer,
            bind_group_layout,
            bind_group,
        }
    }

    pub fn create_buffer(
        device: &wgpu::Device,
        camera: &Camera,
        uniform: &mut CameraUniform,
        projection: &Projection,
    ) -> wgpu::Buffer {
        use wgpu::util::DeviceExt;

        uniform.update_view_projection(&camera, &projection);

        let camera_buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label: Some("Camera Buffer"),
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
                label: Some("camera_bind_group_layout"),
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
            label: Some("camera_bind_group"),
        });

        camera_bind_group
    }
}

// let mut camera_uniform = CameraUniform::new();
// camera_uniform.update_view_projection(&camera);
// let camera_buffer = device.create_buffer_init(
//     &wgpu::util::BufferInitDescriptor {
//         label: Some("Camera Buffer"),
//         contents: bytemuck::cast_slice(&[camera_uniform]),
//         usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
//     }
// );
// let camera_bind_group_layout = camera_uniform.create_bind_group_layout(&device);

pub struct Projection {
    aspect: f32,
    fovy: Rad<f32>,
    znear: f32,
    zfar: f32,
}

impl Projection {
    pub fn new<F: Into<Rad<f32>>>(
        width: u32,
        height: u32,
        fovy: F,
        znear: f32,
        zfar: f32,
    ) -> Self {
        Self {
            aspect: width as f32 / height as f32,
            fovy: fovy.into(),
            znear,
            zfar,
        }
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.aspect = width as f32 / height as f32;
    }

    pub fn calc_matrix(&self) -> Matrix4<f32> {
        OPENGL_TO_WGPU_MATRIX * perspective(self.fovy, self.aspect, self.znear, self.zfar)
    }
}

pub struct Camera {
    // pub eye: Point3<f32>,
    // pub target: Point3<f32>,
    // pub up: Vector3<f32>,
    // pub aspect: f32,
    // pub fovy: f32,
    // pub znear: f32,
    // pub zfar: f32,
    pub position: Point3<f32>,
    pub yaw: Rad<f32>,
    pub pitch: Rad<f32>,
}

impl Camera {
    pub fn new<
        V: Into<Point3<f32>>,
        Y: Into<Rad<f32>>,
        P: Into<Rad<f32>>,
    >(
        position: V,
        yaw: Y,
        pitch: P,
    ) -> Self {
        Self {
            position: position.into(),
            yaw: yaw.into(),
            pitch: pitch.into(),
        }
    }

    pub fn calc_matrix(&self) -> Matrix4<f32> {
        let (sin_pitch, cos_pitch) = self.pitch.0.sin_cos();
        let (sin_yaw, cos_yaw) = self.yaw.0.sin_cos();

        Matrix4::look_to_rh(
            self.position,
            Vector3::new(
                cos_pitch * cos_yaw,
                sin_pitch,
                cos_pitch * sin_yaw
            ).normalize(),
            Vector3::unit_y(),
        )
    }

    // pub fn build_view_projection_matrix(&self) -> Matrix4<f32> {
    //     let view = Matrix4::look_at_rh(self.eye, self.target, self.up);
    //     let projection = perspective(Deg(self.fovy), self.aspect, self.znear, self.zfar);
    //
    //     return OPENGL_TO_WGPU_MATRIX * projection * view;
    // }
}
