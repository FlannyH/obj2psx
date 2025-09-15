use std::{ffi::c_void, mem::size_of};

use gl::types::{GLenum, GLfloat, GLvoid};
use glfw::{Glfw, PWindow};
use log::error;
use memoffset::offset_of;

use crate::psx_structs::CollVertexPSX;

const RESOLUTION: u32 = 32;

pub struct Renderer {
    program: u32,
    vao: u32,
    vbo: u32,
    n_vertices: i32,
    _window: PWindow,
    _glfw: Glfw,
}

impl Renderer {
    pub fn new() -> Self {
        // Set up a basic OpenGL setup
        let mut glfw = glfw::init(glfw::fail_on_errors).unwrap();

        // Create an invisible window
        glfw.window_hint(glfw::WindowHint::Visible(false));
        let (window, _events) = glfw
            .create_window(RESOLUTION, RESOLUTION, "title", glfw::WindowMode::Windowed)
            .expect("Failed to create window.");
        glfw.make_context_current(Some(&window));
        glfw.set_swap_interval(glfw::SwapInterval::None);

        // Init OpenGL
        gl::load_with(|f_name| glfw.get_proc_address_raw(f_name));
        unsafe {
            let error = gl::GetError();
            if error != gl::NO_ERROR {
                panic!();
            }
        }
        let program;

        // Create shader
        unsafe {
            program = gl::CreateProgram();
            Self::load_shader_part(
                gl::VERTEX_SHADER,
                String::from(
                    "
                #version 460

                // Vertex input
                layout (location = 0) in vec3 i_position;

                // View matrix
                layout (location = 0) uniform mat4 u_matrix;
                layout (location = 1) uniform vec3 u_position;

                out vec4 o_position;

                void main() {
                    o_position = u_matrix * vec4(i_position, 1);
                    gl_Position = o_position;
                }
            ",
                ),
                program,
            );
            Self::load_shader_part(
                gl::FRAGMENT_SHADER,
                String::from(
                    "
                #version 460

                in vec4 o_position;
                out vec4 frag_color;

                void main() {
                    gl_FragDepth = o_position.z;
                }
            ",
                ),
                program,
            );

            gl::LinkProgram(program);
            gl::UseProgram(program);
        }
        Self {
            program: program,
            vao: 0,
            vbo: 0,
            n_vertices: 0,
            _window: window,
            _glfw: glfw,
        }
    }

    pub fn load_shader_part(shader_type: GLenum, source: String, program: u32) {
        let source_len = source.len() as i32;

        unsafe {
            // Create shader part
            let shader = gl::CreateShader(shader_type);
            gl::ShaderSource(shader, 1, &source.as_bytes().as_ptr().cast(), &source_len);
            gl::CompileShader(shader);

            // Check for errors
            let mut result = 0;
            let mut log_length = 0;
            gl::GetShaderiv(shader, gl::COMPILE_STATUS, &mut result);
            gl::GetShaderiv(shader, gl::INFO_LOG_LENGTH, &mut log_length);
            let mut error_message: Vec<u8> = vec![0; log_length as usize];
            gl::GetShaderInfoLog(
                shader,
                log_length,
                std::ptr::null_mut(),
                error_message.as_mut_ptr().cast(),
            );

            // Did we get an error?
            if log_length > 0 {
                error!(
                    "Shader compilation error!\n{}",
                    std::str::from_utf8(error_message.as_slice()).unwrap()
                )
            }

            // Attach to program
            gl::AttachShader(program, shader);
        }
    }

    pub fn upload_mesh_to_gpu(&mut self, vertices: &Vec<CollVertexPSX>) {
        // Upload the mesh to the GPU
        unsafe {
            // Generate buffers
            gl::GenVertexArrays(1, &mut self.vao);
            gl::GenBuffers(1, &mut self.vbo);
            gl::BindVertexArray(self.vao);
            gl::BindBuffer(gl::ARRAY_BUFFER, self.vbo);

            // Define vertex layout
            gl::VertexAttribPointer(
                0,
                3,
                gl::SHORT,
                gl::FALSE,
                size_of::<CollVertexPSX>() as i32,
                offset_of!(CollVertexPSX, pos_x) as *const _,
            );
            gl::EnableVertexAttribArray(0);

            // Upload the buffer
            gl::BufferData(
                gl::ARRAY_BUFFER,
                (size_of::<CollVertexPSX>() * vertices.len()) as isize,
                &vertices[0] as *const CollVertexPSX as *const c_void,
                gl::STATIC_DRAW,
            );
        }
        self.n_vertices = vertices.len() as _;
    }

    pub fn is_path_occupied(&mut self, from_position: glam::Vec3, to_target: glam::Vec3, ray_width: f32) -> bool {
        let distance_to_target = from_position.distance(to_target);
        let matrix_proj = glam::Mat4::orthographic_rh(-ray_width, ray_width, -ray_width, ray_width, 0.0, distance_to_target * 2.0);
        // let matrix_proj = glam::Mat4::perspective_rh(PI / 2.0, 1.0, 0.01, 65536.0); // debug view
        let matrix_view = glam::Mat4::look_at_rh(-from_position, -to_target, glam::Vec3::new(0.0, -1.0, 0.0)); // todo: check if this is correct
        let matrix_combined = matrix_proj * matrix_view;
        let mut buffer = vec![0.0f32; (RESOLUTION * RESOLUTION) as usize];
        
        unsafe {
            // Render scene
            gl::UseProgram(self.program);
            gl::ClearDepth(1.0);
            gl::ClearColor(1.0, 1.0, 1.0, 1.0);
            gl::Clear(gl::DEPTH_BUFFER_BIT | gl::COLOR_BUFFER_BIT);
            gl::UniformMatrix4fv(0, 1, gl::FALSE, matrix_combined.as_ref().as_ptr() as *const GLfloat);
            gl::Enable(gl::DEPTH_TEST);
            gl::Enable(gl::CULL_FACE);
            gl::Viewport(0, 0, RESOLUTION as _, RESOLUTION as _);
            gl::DrawArrays(gl::TRIANGLES, 0, self.n_vertices as i32);

            // Get depth buffer to cpu
            gl::ReadPixels(0, 0, RESOLUTION as i32, RESOLUTION as i32, gl::DEPTH_COMPONENT, gl::FLOAT, buffer.as_mut_ptr() as *mut GLvoid);
        }

        // find minimum depth in framebuffer
        let min_depth = buffer.into_iter().reduce(f32::min).unwrap_or(1.0);

        // The orthographic projection matrix takes the depth from [0, distance_to_target * 2.0] to [0, 1], so we just gotta check if the value is smaller than or equal to 0.5
        // So if the minimum depth is bigger than 0.5, the path from position to target is obstructed
        return min_depth <= 0.5
    }
}
