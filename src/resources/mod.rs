use std::io::{BufReader, Cursor};
use std::path::PathBuf;

use cfg_if::cfg_if;

use wgpu::util::DeviceExt;

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

    println!("LoadModel File {:?}", file_name);

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
            println!("DiffX {:?}", m.diffuse_texture);
            if m.diffuse_texture.is_empty() {
                DEFAULT_DIFFUSE_PATH
            } else {
                diffuse_path.to_str().unwrap()
            }
        };
        let normal_path: PathBuf = [basepath.clone(), m.normal_texture.clone().into()].iter().collect();
        let normal_path_str = {
            println!("NormalX {:?}", m.normal_texture);
            if m.normal_texture.is_empty() {
                DEFAULT_NORMAL_PATH
            } else {
                normal_path.to_str().unwrap()
            }
        };

        println!("DiffPath {:?}", diffuse_path_str);
        println!("NormalPath {:?}", normal_path_str);

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
            // println!("MODEL--- {:?}", m);
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