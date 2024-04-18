// Quad Vertex Shader
struct CameraUniform {
  view_position: vec4<f32>,
  view_projection: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> camera: CameraUniform;

struct QuadUniform {
  model: mat4x4<f32>,
};
@group(1) @binding(0)
var<uniform> quad_model: QuadUniform;

struct VertexInput {
    @location(0) position: vec3<f32>,
    @location(1) color: vec4<f32>,
};

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
};

@vertex
fn vs_main(
    model: VertexInput,
) -> VertexOutput {
  var out: VertexOutput;
  var scale = 0.25;
  out.color = vec4<f32>(model.color);
  // out.clip_position = vec4<f32>(model.position, 1.0);
  var clip_pos = vec4<f32>(model.position, 1.0);
  // out.clip_position = camera.view_projection * quad_model.model * vec4<f32>(model.position, 1.0);

  var world_pos = quad_model.model * vec4<f32>(model.position, 1.0);
  // var clip_pos = camera.view_projection * camera.view_position * world_pos;
  var vp = camera.view_position * camera.view_projection;
  // var clip_pos = world_pos * vp;

  out.clip_position = clip_pos;

  return out;
}

// Quad Fragment Shader

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return vec4<f32>(in.color);
}
