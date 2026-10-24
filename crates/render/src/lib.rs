use glow::HasContext;
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

const FLOATS_PER_VERT: usize = 9;

pub struct Renderer {
    program: glow::Program,
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    u_size: Option<glow::UniformLocation>,
    verts: Vec<f32>,
}

impl Renderer {
    pub fn new(gl: &glow::Context, profile: Profile) -> Result<Self, String> {
        let header = match profile {
            Profile::Desktop => "#version 330 core\n",
            Profile::Gles => "#version 300 es\nprecision mediump float;\n",
        };
        unsafe {
            let program = gl.create_program()?;
            let mut shaders = Vec::new();
            for (kind, src) in [(glow::VERTEX_SHADER, VERT), (glow::FRAGMENT_SHADER, FRAG)] {
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
            Ok(Self { program, vao, vbo, u_size, verts: Vec::new() })
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
            gl.clear(glow::COLOR_BUFFER_BIT);
            gl.disable(glow::SCISSOR_TEST);

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

    fn quad(&mut self, x: f32, y: f32, w: f32, h: f32, [r, g, b, a]: [f32; 4], kind: f32) {
        let corners = [(x, y, -1.0, -1.0), (x + w, y, 1.0, -1.0), (x + w, y + h, 1.0, 1.0), (x, y + h, -1.0, 1.0)];
        for i in [0, 1, 2, 0, 2, 3] {
            let (px, py, lx, ly) = corners[i];
            self.verts.extend_from_slice(&[px, py, lx, ly, r, g, b, a, kind]);
        }
    }
}
