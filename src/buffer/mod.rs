pub struct Buffer {
    buffer: wgpu::Buffer,
    label: Option<&str>,
}

impl Buffer {
    pub fn new(device: &wgpu::Device, label: Option<&str>) -> Self {
        let buffer = create_buffer(device, buffer_data, label);

        Self {
            buffer,
            label,
        }
    }

    pub fn create_buffer(device: &wgpu::Device) -> wgpu::Buffer {
        use wgpu::util::DeviceExt;

        let buffer = device.create_buffer_init(
            &wgpu::util::BufferInitDescriptor {
                label,
                contents: bytemuck::cast_slice(&buffer_data),
                usuage: wgpu::BufferUsage::VERTEX,
            }
        );

        buffer
    }
}
