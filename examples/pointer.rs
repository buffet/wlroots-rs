#[macro_use]
extern crate wlroots;

use std::cell::{Cell, RefCell};
use std::rc::Rc;
use wlroots::{AxisEvent, ButtonEvent, Compositor, CompositorBuilder, Cursor, InputManagerHandler,
              KeyEvent, KeyboardHandler, MotionEvent, OutputBuilder, OutputBuilderResult,
              OutputHandler, OutputLayout, OutputManagerHandler, PointerHandler, XCursorTheme};
use wlroots::types::{KeyboardHandle, OutputHandle, PointerHandle};
use wlroots::wlroots_sys::gl;
use wlroots::wlroots_sys::wlr_button_state::WLR_BUTTON_RELEASED;
use wlroots::xkbcommon::xkb::keysyms::KEY_Escape;

struct OutputManager {
    color: Rc<Cell<[f32; 4]>>,
    cursor: Rc<RefCell<Cursor>>
}

struct Output {
    color: Rc<Cell<[f32; 4]>>
}

struct InputManager {
    color: Rc<Cell<[f32; 4]>>,
    cursor: Rc<RefCell<Cursor>>
}

struct Pointer {
    color: Rc<Cell<[f32; 4]>>,
    default_color: [f32; 4],
    cursor: Rc<RefCell<Cursor>>
}

struct ExKeyboardHandler;

impl OutputManagerHandler for OutputManager {
    fn output_added<'output>(&mut self,
                             _: &mut Compositor,
                             builder: OutputBuilder<'output>)
                             -> Option<OutputBuilderResult<'output>> {
        let mut result = builder.build_best_mode(Output { color: self.color.clone() });
        let mut cursor;
        {
            let output = &mut result.output;
            cursor = self.cursor.borrow_mut();
            {
                let xcursor = cursor.xcursor().expect("XCursor was not set!");
                let image = &xcursor.images()[0];
                // TODO use output config if present instead of auto
                let layout = cursor
                    .output_layout()
                    .as_ref()
                    .expect("Could not get output layout");
                output.add_layout_auto(layout.clone());
                if output.set_cursor(image).is_err() {
                    wlr_log!(L_DEBUG, "Failed to set hardware cursor");
                    return None;
                }
            }
        }
        let (x, y) = cursor.coords();
        // https://en.wikipedia.org/wiki/Mouse_warping
        cursor.warp(None, x, y);
        Some(result)
    }
}

impl KeyboardHandler for ExKeyboardHandler {
    fn on_key(&mut self,
              compositor: &mut Compositor,
              _: &mut KeyboardHandle,
              key_event: &mut KeyEvent) {
        for key in key_event.input_keys() {
            if key == KEY_Escape {
                compositor.terminate()
            }
        }
    }
}

impl PointerHandler for Pointer {
    fn on_motion(&mut self, _: &mut Compositor, _: &mut PointerHandle, event: &MotionEvent) {
        let (delta_x, delta_y) = event.delta();
        self.cursor
            .borrow_mut()
            .move_to(&event.device(), delta_x, delta_y);
    }

    fn on_button(&mut self, _: &mut Compositor, _: &mut PointerHandle, event: &ButtonEvent) {
        if event.state() == WLR_BUTTON_RELEASED {
            self.color.set(self.default_color.clone())
        } else {
            let mut red: [f32; 4] = [0.25, 0.25, 0.25, 1.0];
            red[event.button() as usize % 3] = 1.0;
            self.color.set(red);
        }
    }

    fn on_axis(&mut self, _: &mut Compositor, _: &mut PointerHandle, event: &AxisEvent) {
        for color_byte in &mut self.default_color[..3] {
            *color_byte += if event.delta() > 0.0 { -0.05 } else { 0.05 };
            if *color_byte > 1.0 {
                *color_byte = 1.0
            }
            if *color_byte < 0.0 {
                *color_byte = 0.0
            }
        }
        self.color.set(self.default_color)
    }
}

impl OutputHandler for Output {
    fn output_frame(&mut self, _: &mut Compositor, output: &mut OutputHandle) {
        output.make_current();
        unsafe {
            gl::ClearColor(self.color.get()[0],
                           self.color.get()[1],
                           self.color.get()[2],
                           1.0);
            gl::Clear(gl::COLOR_BUFFER_BIT);
        }
        output.swap_buffers();
    }
}

impl InputManagerHandler for InputManager {
    fn pointer_added(&mut self,
                     _: &mut Compositor,
                     _: &mut PointerHandle)
                     -> Option<Box<PointerHandler>> {
        Some(Box::new(Pointer {
                          color: self.color.clone(),
                          default_color: self.color.get(),
                          cursor: self.cursor.clone()
                      }))
    }

    fn keyboard_added(&mut self,
                      _: &mut Compositor,
                      _: &mut KeyboardHandle)
                      -> Option<Box<KeyboardHandler>> {
        Some(Box::new(ExKeyboardHandler))
    }
}

fn managers(mut cursor: Cursor) -> (OutputManager, InputManager) {
    let layout = Rc::new(RefCell::new(OutputLayout::new()));
    // TODO Ensure this can be safe...
    // e.g what's stopping me from simply dropping layout now that I gave it to
    // cursor?
    cursor.attach_output_layout(layout);
    let cursor = Rc::new(RefCell::new(cursor));
    let color = Rc::new(Cell::new([0.25, 0.25, 0.25, 1.0]));
    (OutputManager {
         color: color.clone(),
         cursor: cursor.clone()
     },
     InputManager {
         color: color.clone(),
         cursor: cursor.clone()
     })
}

fn main() {
    let mut cursor = Cursor::new().expect("Could not create cursor");
    let xcursor_theme = XCursorTheme::load_theme(None, 16).expect("Could not load theme");
    let xcursor = xcursor_theme
        .get_cursor("left_ptr".into())
        .expect("Could not load cursor from theme");
    cursor.set_xcursor(Some(xcursor));

    let (output_manager, input_manager) = managers(cursor);
    let compositor = CompositorBuilder::new()
        .build_auto((), Box::new(input_manager), Box::new(output_manager));
    compositor.run();
}
