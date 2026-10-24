use std::collections::HashMap;

use glow::HasContext;
use twinbit_game::scene3d::{MeshId, Scene3d, Vertex3};
use twinbit_game::{Canvas, SCREEN_H, SCREEN_W, Shape};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Profile {
    /// OpenGL 3.3 core (macOS).
    Desktop,
    /// OpenGL ES 3.0 (Mali on the device).
    Gles,
}

const VERT: &str = r#"
layout(location = 0) in vec2 a_pos;
layout(location = 1) in vec2 a_local;
layout(location = 2) in vec4 a_color;
layout(location = 3) in float a_kind;
uniform vec2 u_size;
out vec2 v_local;
out vec4 v_color;
out float v_kind;
void main() {
    gl_Position = vec4(a_pos.x / u_size.x * 2.0 - 1.0, 1.0 - a_pos.y / u_size.y * 2.0, 0.0, 1.0);
    v_local = a_local;
    v_color = a_color;
    v_kind = a_kind;
}
"#;

const FRAG: &str = r#"
in vec2 v_local;
in vec4 v_color;
in float v_kind;
out vec4 o_color;
void main() {
    float a = 1.0;
    if (v_kind > 0.5) {
        float d = length(v_local);
        a = 1.0 - smoothstep(1.0 - fwidth(d), 1.0, d);
    }
    o_color = vec4(v_color.rgb, v_color.a * a);
}
"#;

const MESH_VERT: &str = r#"
layout(location = 0) in vec3 a_pos;
layout(location = 1) in vec3 a_normal;
layout(location = 2) in vec3 a_color;
uniform mat4 u_view_proj;
uniform mat4 u_model;
out vec3 v_world;
out vec3 v_normal;
out vec3 v_color;
void main() {
    vec4 world = u_model * vec4(a_pos, 1.0);
    gl_Position = u_view_proj * world;
    v_world = world.xyz;
    v_normal = mat3(u_model) * a_normal;
    v_color = a_color;
}
"#;

const MESH_FRAG: &str = r#"
in vec3 v_world;
in vec3 v_normal;
in vec3 v_color;
uniform vec3 u_light;
uniform vec3 u_eye;
out vec4 o_color;
void main() {
    vec3 n = normalize(v_normal);
    vec3 h = normalize(u_light + normalize(u_eye - v_world));
    float diffuse = max(dot(n, u_light), 0.0);
    float spec = pow(max(dot(n, h), 0.0), 32.0) * 0.25;
    o_color = vec4(v_color * (0.35 + 0.65 * diffuse) + spec, 1.0);
}
"#;

const FLOATS_PER_VERT: usize = 9;

struct Mesh {
    vao: glow::VertexArray,
    count: i32,
}

pub struct Renderer {
    program: glow::Program,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    u_size: Option<glow::UniformLocation>,
    verts: Vec<f32>,
    mesh_program: glow::Program,
    meshes: HashMap<MeshId, Mesh>,
}

unsafe fn link(gl: &glow::Context, header: &str, vert: &str, frag: &str) -> Result<glow::Program, String> {
    unsafe {
        let program = gl.create_program()?;
        let mut shaders = Vec::new();
        for (kind, src) in [(glow::VERTEX_SHADER, vert), (glow::FRAGMENT_SHADER, frag)] {
            let shader = gl.create_shader(kind)?;
            gl.shader_source(shader, &format!("{header}{src}"));
            gl.compile_shader(shader);
            if !gl.get_shader_compile_status(shader) {
                return Err(gl.get_shader_info_log(shader));
            }
            gl.attach_shader(program, shader);
            shaders.push(shader);
        }
        gl.link_program(program);
        if !gl.get_program_link_status(program) {
            return Err(gl.get_program_info_log(program));
        }
        for shader in shaders {
            gl.detach_shader(program, shader);
            gl.delete_shader(shader);
        }
        Ok(program)
    }
}

unsafe fn upload_mesh(gl: &glow::Context, verts: &[Vertex3]) -> Result<Mesh, String> {
    unsafe {
        let floats: Vec<f32> = verts.iter().flat_map(|v| v.pos.into_iter().chain(v.normal).chain(v.color)).collect();
        let vao = gl.create_vertex_array()?;
        let vbo = gl.create_buffer()?;
        gl.bind_vertex_array(Some(vao));
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        let bytes = std::slice::from_raw_parts(floats.as_ptr().cast::<u8>(), floats.len() * 4);
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::STATIC_DRAW);
        for (loc, offset) in [(0, 0), (1, 3), (2, 6)] {
            gl.enable_vertex_attrib_array(loc);
            gl.vertex_attrib_pointer_f32(loc, 3, glow::FLOAT, false, 9 * 4, offset * 4);
        }
        Ok(Mesh { vao, count: verts.len() as i32 })
    }
}

impl Renderer {
    pub fn new(gl: &glow::Context, profile: Profile) -> Result<Self, String> {
        let header = match profile {
            Profile::Desktop => "#version 330 core\n",
            Profile::Gles => "#version 300 es\nprecision mediump float;\n",
        };
        unsafe {
            let program = link(gl, header, VERT, FRAG)?;
            let mesh_program = link(gl, header, MESH_VERT, MESH_FRAG)?;
            let mut meshes = HashMap::new();
            for id in MeshId::ALL {
                meshes.insert(id, upload_mesh(gl, &id.build())?);
            }

            let vao = gl.create_vertex_array()?;
            let vbo = gl.create_buffer()?;
            gl.bind_vertex_array(Some(vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
            let stride = (FLOATS_PER_VERT * 4) as i32;
            for (loc, size, offset) in [(0, 2, 0), (1, 2, 2), (2, 4, 4), (3, 1, 8)] {
                gl.enable_vertex_attrib_array(loc);
                gl.vertex_attrib_pointer_f32(loc, size, glow::FLOAT, false, stride, offset * 4);
            }

            let u_size = gl.get_uniform_location(program, "u_size");
            Ok(Self { program, vao, vbo, u_size, verts: Vec::new(), mesh_program, meshes })
        }
    }

    /// Draws one screen's canvas into `viewport` (x, y, w, h in framebuffer pixels, GL origin).
    pub fn draw(&mut self, gl: &glow::Context, canvas: &Canvas, viewport: [i32; 4]) {
        self.verts.clear();
        for shape in &canvas.shapes {
            match *shape {
                Shape::Rect { x, y, w, h, color } => self.quad(x, y, w, h, color, 0.0),
                Shape::Circle { x, y, r, color } => self.quad(x - r, y - r, 2.0 * r, 2.0 * r, color, 1.0),
            }
        }
        let [x, y, w, h] = viewport;
        let [r, g, b, a] = canvas.clear;
        unsafe {
            gl.viewport(x, y, w, h);
            gl.enable(glow::SCISSOR_TEST);
            gl.scissor(x, y, w, h);
            gl.clear_color(r, g, b, a);
            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
            gl.disable(glow::SCISSOR_TEST);
            if let Some(scene) = &canvas.scene {
                self.draw_scene(gl, scene);
            }

            gl.enable(glow::BLEND);
            gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
            gl.use_program(Some(self.program));
            gl.uniform_2_f32(self.u_size.as_ref(), SCREEN_W, SCREEN_H);
            gl.bind_vertex_array(Some(self.vao));
            gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
            let bytes = std::slice::from_raw_parts(self.verts.as_ptr().cast::<u8>(), self.verts.len() * 4);
            gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytes, glow::STREAM_DRAW);
            gl.draw_arrays(glow::TRIANGLES, 0, (self.verts.len() / FLOATS_PER_VERT) as i32);
        }
    }

    fn draw_scene(&self, gl: &glow::Context, scene: &Scene3d) {
        unsafe {
            gl.enable(glow::DEPTH_TEST);
            gl.depth_func(glow::LEQUAL);
            gl.disable(glow::BLEND);
            gl.use_program(Some(self.mesh_program));
            let loc = |name| gl.get_uniform_location(self.mesh_program, name);
            gl.uniform_matrix_4_f32_slice(loc("u_view_proj").as_ref(), false, &scene.view_proj.to_cols_array());
            gl.uniform_3_f32_slice(loc("u_light").as_ref(), &scene.light.to_array());
            gl.uniform_3_f32_slice(loc("u_eye").as_ref(), &scene.eye.to_array());
            let u_model = loc("u_model");
            for (id, model) in &scene.draws {
                let mesh = &self.meshes[id];
                if id.is_decal() {
                    gl.enable(glow::POLYGON_OFFSET_FILL);
                    gl.polygon_offset(-2.0, -64.0);
                } else {
                    gl.disable(glow::POLYGON_OFFSET_FILL);
                }
                gl.uniform_matrix_4_f32_slice(u_model.as_ref(), false, &model.to_cols_array());
                gl.bind_vertex_array(Some(mesh.vao));
                gl.draw_arrays(glow::TRIANGLES, 0, mesh.count);
            }
            gl.disable(glow::POLYGON_OFFSET_FILL);
            gl.disable(glow::DEPTH_TEST);
        }
    }

    fn quad(&mut self, x: f32, y: f32, w: f32, h: f32, [r, g, b, a]: [f32; 4], kind: f32) {
        let corners = [(x, y, -1.0, -1.0), (x + w, y, 1.0, -1.0), (x + w, y + h, 1.0, 1.0), (x, y + h, -1.0, 1.0)];
        for i in [0, 1, 2, 0, 2, 3] {
            let (px, py, lx, ly) = corners[i];
            self.verts.extend_from_slice(&[px, py, lx, ly, r, g, b, a, kind]);
        }
    }
}
