use std::io::{BufReader, Cursor};
use std::path::PathBuf;

use cfg_if::cfg_if;

use wgpu::util::DeviceExt;

use gltf::Gltf;

use crate::{
    model::{
        Model,
        ModelVertex,
        Mesh,
        Material,
    },
    texture::Texture,
};

#[cfg(target_arch = "wasm32")]
fn format_url(file_name: &str) -> reqwest::Url {
    let window = web_sys::window().unwrap();
    let location = window.location();
    let mut origin = location.origin().unwrap();

    if !origin.ends_with("assets") {
        origin = format!("{}/assets", origin);
    }

    let base = reqwest::Url::parse(&format!("{}/", origin,)).unwrap();

    base.join(file_name).unwrap()
}

pub async fn load_string(file_name: &str) -> anyhow::Result<String> {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let url = format_url(file_name);
            let txt = reqwest::get(url)
                .await?
                .text()
                .await?;
        } else {
            let path = std::path::Path::new(env!("OUT_DIR"))
                .join("assets")
                .join(file_name);
            let txt = std::fs::read_to_string(path)?;
        }
    }

    Ok(txt)
}

pub async fn load_binary(file_name: &str) -> anyhow::Result<Vec<u8>> {
    cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            let url = format_url(file_name);
            let data = reqwest::get(url)
                .await?
                .bytes()
                .await?
                .to_vec();
        } else {
            let path = std::path::Path::new(env!("OUT_DIR"))
                .join("assets")
                .join(file_name);
            let data = std::fs::read(path)?;
        }
    }

    Ok(data)
}

pub async fn load_texture(
    file_name: &str,
    is_normal_map: bool,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
) -> anyhow::Result<Texture> {
    let data = load_binary(file_name).await?;

    Texture::from_bytes(device, queue, &data, file_name, is_normal_map)
}

const DEFAULT_DIFFUSE_PATH: &str = "meshes/core/empty-texture.png";
const DEFAULT_NORMAL_PATH: &str = "meshes/core/empty-normal.png";

pub async fn load_model_gltf(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> anyhow::Result<Model> {
    let gltf_text = load_string(file_name).await?;
    let gltf_cursor = Cursor::new(gltf_text);
    let gltf_reader = BufReader::new(gltf_cursor);
    let gltf = Gltf::from_reader(gltf_reader)?;

    let mut basepath = PathBuf::from(file_name);
    basepath.pop();

    // load Buffers
    let mut buffer_data = Vec::new();
    for buffer in gltf.buffers() {
        match buffer.source() {
            gltf::buffer::Source::Bin => {
                if let Some(blob) = gltf.blob.as_deref() {
                    buffer_data.push(blob.into());
                };
            }
            gltf::buffer::Source::Uri(uri) => {
                let binary_path: PathBuf = [basepath.clone(), uri.into()].iter().collect();//m.diffuse_texture.clone().into()].iter().collect();
                let bin = load_binary(binary_path.to_str().unwrap()).await.expect("File Not Found");
                buffer_data.push(bin);
            }
        }
    }

    let mut materials = Vec::new();
    for material in gltf.materials() {
        let pbr = material.pbr_metallic_roughness();
        let texture_source = &pbr.base_color_texture()
            .map(|tex| {
                tex.texture().source().source()
            })
            .expect("Issue Finding Texture Source");
        let is_normal_map = false;
        let default_normal_texture = load_texture(DEFAULT_NORMAL_PATH, true, device, queue).await?;
        match texture_source {
            gltf::image::Source::View { view, mime_type } => {
                // Image texture data is in the binary
                let diffuse_texture = Texture::from_bytes(
                    device,
                    queue,
                    &buffer_data[view.buffer().index()],
                    file_name,
                    is_normal_map,
                )
                .expect("Issue loading Diffuse Texture");

                let mat = Material::new(
                    device,
                    material.name().unwrap_or("Default Material"),
                    diffuse_texture,
                    default_normal_texture,
                    layout,
                );
                materials.push(mat);
            }
            gltf::image::Source::Uri { uri, mime_type } => {
                let full_path: PathBuf = [basepath.clone(), uri.into()].iter().collect();
                let full_uri = full_path.to_str().unwrap();
                // Image texture data is in a separate image file
                let diffuse_texture = load_texture(full_uri, is_normal_map, device, queue).await?;

                let mat = Material::new(
                    device,
                    material.name().unwrap_or("Default Material"),
                    diffuse_texture,
                    default_normal_texture,
                    layout,
                );
                materials.push(mat);
            }
        }
    }

    let mut meshes = Vec::new();

    for scene in gltf.scenes() {
        for node in scene.nodes() {
            let mesh = node.mesh().expect("Unable to load Mesh");
            let primitives = mesh.primitives();

            primitives.for_each(|primitive| {
                let reader = primitive.reader(|buffer| {
                    Some(&buffer_data[buffer.index()])
                });

                let mut vertices = Vec::new();

                if let Some(vertex_attribute) = reader.read_positions() {
                    vertex_attribute.for_each(|vertex| {
                        vertices.push(ModelVertex {
                            position: vertex,
                            tex_coords: Default::default(),
                            normal: Default::default(),
                            tangent: Default::default(),
                            bitangent: Default::default(),
                        })
                    });
                }

                if let Some(normal_attribute) = reader.read_normals() {
                    let mut normal_index = 0;
                    normal_attribute.for_each(|normal| {
                        vertices[normal_index].normal = normal;

                        normal_index += 1;
                    });
                }

                if let Some(tex_coord_attribute) = reader.read_tex_coords(0).map(|v| v.into_f32()) {
                    let mut tex_coord_index = 0;
                    tex_coord_attribute.for_each(|tex_coord| {
                        // need to flip/invert the y-axis of UV tex coords for wgpu/WebGPU
                        let reverse_y_tex_coords = [tex_coord[0], 1.0 - tex_coord[1]];
                        vertices[tex_coord_index].tex_coords = reverse_y_tex_coords;

                        tex_coord_index += 1;
                    });
                }

                let mut indices = Vec::new();
                if let Some(indices_raw) = reader.read_indices() {
                    indices.append(&mut indices_raw.into_u32().collect::<Vec<u32>>());
                }

                calculate_normal_tangents(&indices, &mut vertices);

                let vertex_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("{:?} Vertex Buffer", file_name)),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                });

                let index_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("{:?} Index Buffer", file_name)),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                });

                let material_name = primitive.material().name().unwrap_or_default();
                let material_index = materials.iter().position(|m| {
                    m.name == material_name
                });

                meshes.push(Mesh {
                    name: file_name.to_string(),
                    vertex_buffer,
                    index_buffer,
                    num_elements: indices.len() as u32,
                    material: material_index.unwrap_or(0),
                });
            });
        }
    }

    Ok(Model { meshes, materials })
}

pub async fn load_model(
    file_name: &str,
    device: &wgpu::Device,
    queue: &wgpu::Queue,
    layout: &wgpu::BindGroupLayout,
) -> anyhow::Result<Model> {
    let obj_text = load_string(file_name).await?;
    let obj_cursor = Cursor::new(obj_text);
    let mut obj_reader = BufReader::new(obj_cursor);
    let mut basepath = PathBuf::from(file_name);
    basepath.pop();


    let (models, obj_materials) = tobj::load_obj_buf_async(
        &mut obj_reader,
        &tobj::LoadOptions {
            triangulate: true,
            single_index: true,
            ..Default::default()
        },
        |p| async move {
            let mut basepath = PathBuf::from(file_name);
            basepath.pop();
            let fullpath: PathBuf = [basepath, p.clone().into()].iter().collect();
            let fullpath_str = fullpath.to_str().unwrap();
            let mat_text = load_string(&fullpath_str).await.unwrap();

            tobj::load_mtl_buf(&mut BufReader::new(Cursor::new(mat_text)))
        },
    ).await?;

    let mut materials = Vec::new();

    for m in obj_materials? {
        let diffuse_path: PathBuf = [basepath.clone(), m.diffuse_texture.clone().into()].iter().collect();
        let diffuse_path_str = {
            if m.diffuse_texture.is_empty() {
                DEFAULT_DIFFUSE_PATH
            } else {
                diffuse_path.to_str().unwrap()
            }
        };
        let normal_path: PathBuf = [basepath.clone(), m.normal_texture.clone().into()].iter().collect();
        let normal_path_str = {
            if m.normal_texture.is_empty() {
                DEFAULT_NORMAL_PATH
            } else {
                normal_path.to_str().unwrap()
            }
        };

        let diffuse_texture = load_texture(&diffuse_path_str, false, device, queue).await?;
        let normal_texture = load_texture(&normal_path_str, true, device, queue).await?;

        let material = Material::new(
            device,
            &m.name,
            diffuse_texture,
            normal_texture,
            layout,
        );

        materials.push(material);
    }

    let meshes = models
        .into_iter()
        .map(|m| {
            let mut vertices = (0..m.mesh.positions.len() / 3)
                .map(|i| ModelVertex {
                    position: [
                        m.mesh.positions[i * 3],
                        m.mesh.positions[i * 3 + 1],
                        m.mesh.positions[i * 3 + 2],
                    ],
                    tex_coords: [
                        m.mesh.texcoords[i * 2],
                        m.mesh.texcoords[i * 2 + 1], // 1 - y reverse y
                    ],
                    normal: [
                        m.mesh.normals[i * 3],
                        m.mesh.normals[i * 3 + 1],
                        m.mesh.normals[i * 3 + 2],
                    ],
                    // we'll calculate these later
                    tangent: [0.0; 3],
                    bitangent: [0.0; 3],
                })
                .collect::<Vec<_>>();

            let indices = &m.mesh.indices;

            calculate_normal_tangents(indices, &mut vertices);

            let vertex_buffer = device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("{:?} Vertex Buffer", file_name)),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                }
            );

            let index_buffer = device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some(&format!("{:?} Index Buffer", file_name)),
                    contents: bytemuck::cast_slice(&m.mesh.indices),
                    usage: wgpu::BufferUsages::INDEX,
                }
            );

            Mesh {
                name: file_name.to_string(),
                vertex_buffer,
                index_buffer,
                num_elements: m.mesh.indices.len() as u32,
                material: m.mesh.material_id.unwrap_or(0),
            }
        })
        .collect::<Vec<_>>();

    Ok(Model { meshes, materials })
}

pub fn calculate_normal_tangents(indices: &Vec<u32>, vertices: &mut Vec<ModelVertex>) {
    let mut triangles_included = vec![0; vertices.len()];

    // operate on triangles so iterate of chunks of 3
    for c in indices.chunks(3) {
        let v0 = vertices[c[0] as usize];
        let v1 = vertices[c[1] as usize];
        let v2 = vertices[c[2] as usize];

        let pos0: cgmath::Vector3<_> = v0.position.into();
        let pos1: cgmath::Vector3<_> = v1.position.into();
        let pos2: cgmath::Vector3<_> = v2.position.into();

        let uv0: cgmath::Vector2<_> = v0.tex_coords.into();
        let uv1: cgmath::Vector2<_> = v1.tex_coords.into();
        let uv2: cgmath::Vector2<_> = v2.tex_coords.into();

        // calculate edges of triangles
        let delta_pos1 = pos1 - pos0;
        let delta_pos2 = pos2 - pos0;

        // this will give direction to calculate tangent and bitangent
        let delta_uv1 = uv1 - uv0;
        let delta_uv2 = uv2 - uv0;

        // Solving the following system of equations will
        // give us the tangent and bitangent.
        //     delta_pos1 = delta_uv1.x * T + delta_u.y * B
        //     delta_pos2 = delta_uv2.x * T + delta_uv2.y * B
        let r = 1.0 / (delta_uv1.x * delta_uv2.y - delta_uv1.y * delta_uv2.x);
        let tangent = (delta_pos1 * delta_uv2.y - delta_pos2 * delta_uv1.y) * r;
        // flip the bitangent to enable right-handed normal
        // maps with wgpu texture coordinate system
        let bitangent = (delta_pos2 * delta_uv1.x - delta_pos1 * delta_uv2.x) * -r;

        // use same tangent/bitangent for each vertex in the triangle
        vertices[c[0] as usize].tangent = (tangent + cgmath::Vector3::from(vertices[c[0] as usize].tangent)).into();
        vertices[c[1] as usize].tangent = (tangent + cgmath::Vector3::from(vertices[c[1] as usize].tangent)).into();
        vertices[c[2] as usize].tangent = (tangent + cgmath::Vector3::from(vertices[c[2] as usize].tangent)).into();
        vertices[c[0] as usize].bitangent = (bitangent + cgmath::Vector3::from(vertices[c[0] as usize].tangent)).into();
        vertices[c[1] as usize].bitangent = (bitangent + cgmath::Vector3::from(vertices[c[1] as usize].tangent)).into();
        vertices[c[2] as usize].bitangent = (bitangent + cgmath::Vector3::from(vertices[c[2] as usize].tangent)).into();

        // used to average tangent/bitangent
        triangles_included[c[0] as usize] += 1;
        triangles_included[c[1] as usize] += 1;
        triangles_included[c[2] as usize] += 1;
    }

    // average the tangents/bitangents
    for(i, n) in triangles_included.into_iter().enumerate() {
        let denom = 1.0 / n as f32;
        let mut v = vertices[i];

        v.tangent = (cgmath::Vector3::from(v.tangent) * denom).into();
        v.bitangent = (cgmath::Vector3::from(v.bitangent) * denom).into();
    }
}
