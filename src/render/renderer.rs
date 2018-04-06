//! TODO Documentation

use std::time::Duration;

use libc::{c_float, c_int, c_void};

use {Area, Output, PixmanRegion};
use render::Texture;
use wlroots_sys::{wl_shm_format, wlr_backend, wlr_backend_get_egl, wlr_render_ellipse_with_matrix,
                  wlr_render_quad_with_matrix, wlr_render_rect, wlr_render_texture,
                  wlr_render_texture_with_matrix, wlr_renderer, wlr_renderer_begin,
                  wlr_renderer_clear, wlr_renderer_destroy, wlr_renderer_end,
                  wlr_texture_from_pixels, wlr_gles2_renderer_create};

/// A generic interface for rendering to the screen.
///
/// Note that it will technically be possible to have multiple renderers
/// at the same time.
#[derive(Debug)]
pub struct GenericRenderer {
    renderer: *mut wlr_renderer
}

/// The state machine type that allows you to manipulate a screen and
/// its buffer.
///
/// When this structure is dropped it automatically calls wlr_renderer_end
/// and swaps the buffers.
#[derive(Debug)]
pub struct Renderer<'output> {
    renderer: *mut wlr_renderer,
    pub damage: Option<(PixmanRegion, Duration)>,
    pub output: &'output mut Output
}

impl GenericRenderer {
    /// Make a gles2 renderer.
    pub(crate) unsafe fn gles2_renderer(backend: *mut wlr_backend) -> Self {
        let egl = wlr_backend_get_egl(backend);
        if egl.is_null() {
            panic!("EGL not available for this backend");
        }
        let renderer = wlr_gles2_renderer_create(egl);
        if renderer.is_null() {
            panic!("Could not construct GLES2 renderer");
        }
        GenericRenderer { renderer }
    }

    /// Make the `Renderer` state machine type.
    ///
    /// This automatically makes the given output the current output.
    pub fn render<'output, T: Into<Option<(PixmanRegion, Duration)>>>
                  (&mut self,
                  output: &'output mut Output,
                  damage: T)
                  -> Renderer<'output> {
        unsafe {
            output.make_current();
            let (width, height) = output.size();
            wlr_renderer_begin(self.renderer, width, height);
            Renderer { renderer: self.renderer,
                       damage: damage.into(),
                       output }
        }
    }

    /// Create a texture using this renderer.
    pub fn create_texture_from_pixels(&mut self,
                                      format: wl_shm_format,
                                      stride: u32,
                                      width: u32,
                                      height: u32,
                                      data: &[u8])
                                      -> Option<Texture> {
        unsafe {
            create_texture_from_pixels(self.renderer,
                                       format,
                                       stride,
                                       width,
                                       height,
                                       data.as_ptr() as _)
        }
    }

    pub(crate) unsafe fn as_ptr(&self) -> *mut wlr_renderer {
        self.renderer
    }
}

impl Drop for GenericRenderer {
    fn drop(&mut self) {
        unsafe { wlr_renderer_destroy(self.renderer) }
    }
}

impl<'output> Renderer<'output> {
    pub fn clear(&mut self, float: [f32; 4]) {
        unsafe { wlr_renderer_clear(self.renderer, float.as_ptr()) }
    }

    /// Renders the requseted texture.
    pub fn render_texture(&mut self,
                          texture: &Texture,
                          projection: [f32; 9],
                          x: c_int,
                          y: c_int,
                          alpha: c_float)
                          -> bool {
        unsafe {
            wlr_render_texture(self.renderer,
                               texture.as_ptr(),
                               projection.as_ptr(),
                               x,
                               y,
                               alpha)
        }
    }

    /// Renders the requested texture using the provided matrix. A typical texture
    /// rendering goes like so:
    ///
    /// TODO FIXME Show how the typical rendering goes in Rust.
    ///
    /// ```c
    /// struct wlr_renderer *renderer;
    /// struct wlr_texture *texture;
    /// float projection[16];
    /// float matrix[16];
    /// wlr_texture_get_matrix(texture, &matrix, &projection, 123, 321);
    /// wlr_render_texture_with_matrix(renderer, texture, &matrix);
    /// ```
    ///
    /// This will render the texture at <123, 321>.
    pub fn render_texture_with_matrix(&mut self, texture: &Texture, matrix: [f32; 9]) -> bool {
        // TODO FIXME Add alpha as param
        unsafe {
            wlr_render_texture_with_matrix(self.renderer, texture.as_ptr(), matrix.as_ptr(), 1.0)
        }
    }

    /// Renders a solid quad in the specified color.
    pub fn render_colored_quad(&mut self, color: [f32; 4], matrix: [f32; 9]) {
        unsafe { wlr_render_quad_with_matrix(self.renderer, color.as_ptr(), matrix.as_ptr()) }
    }

    /// Renders a solid ellipse in the specified color.
    pub fn render_colored_ellipse(&mut self, color: [f32; 4], matrix: [f32; 9]) {
        unsafe { wlr_render_ellipse_with_matrix(self.renderer, color.as_ptr(), matrix.as_ptr()) }
    }

    /// Renders a solid rectangle in the specified color.
    pub fn render_colored_rect(&mut self, area: Area, color: [f32; 4], matrix: [f32; 9]) {
        unsafe { wlr_render_rect(self.renderer, &area.into(), color.as_ptr(), matrix.as_ptr()) }
    }
}

impl<'output> Drop for Renderer<'output> {
    fn drop(&mut self) {
        unsafe {
            wlr_renderer_end(self.renderer);
            if let Some((mut damage, when)) = self.damage.take() {
                self.output.swap_buffers(Some(when), Some(&mut damage));
            } else {
                self.output.swap_buffers(None, None);
            }
        }
    }
}

unsafe fn create_texture_from_pixels(renderer: *mut wlr_renderer,
                                     format: wl_shm_format,
                                     stride: u32,
                                     width: u32,
                                     height: u32,
                                     // TODO Slice of u8? It's a void*, hmm
                                     data: *const c_void)
                                     -> Option<Texture> {
    let texture = wlr_texture_from_pixels(renderer, format, stride, width, height, data);
    if texture.is_null() {
        None
    } else {
        Some(Texture::from_ptr(texture))
    }
}
