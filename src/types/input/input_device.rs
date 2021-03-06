use std::{cell::Cell, rc::Weak};

use libc::{c_double, c_uint};
use wlroots_sys::{wlr_input_device, wlr_input_device_pointer, wlr_input_device_type,
                  wlr_input_device_type::*};

use {input::{keyboard, pointer, touch, tablet_pad, tablet_tool},
     utils::c_to_rust_string};
pub(crate) use manager::input_manager::Manager;

/// A handle to an input device.
pub enum Handle {
    Keyboard(keyboard::Handle),
    Pointer(pointer::Handle),
    Touch(touch::Handle),
    TabletPad(tablet_pad::Handle),
    TabletTool(tablet_tool::Handle)
}

pub(crate) struct InputState {
    pub(crate) handle: Weak<Cell<bool>>,
    pub(crate) device: Device
}

/// Wrapper for wlr_input_device
#[derive(Debug, Eq, PartialEq, Hash, Ord, PartialOrd)]
pub struct Device {
    pub(crate) device: *mut wlr_input_device
}

impl Device {
    /// Just like `std::clone::Clone`, but unsafe.
    ///
    /// # Unsafety
    /// This is unsafe because the user should not be able to clone
    /// this type out because it isn't bound by anything but the underlying
    /// pointer could be removed at any time.
    ///
    /// This isn't exposed to the user, but still marked as `unsafe` to reduce
    /// possible bugs from using this.
    pub(crate) unsafe fn clone(&self) -> Self {
        Device { device: self.device }
    }

    pub fn vendor(&self) -> c_uint {
        unsafe { (*self.device).vendor }
    }

    pub fn product(&self) -> c_uint {
        unsafe { (*self.device).product }
    }

    pub fn name(&self) -> Option<String> {
        unsafe { c_to_rust_string((*self.device).name) }
    }

    pub fn output_name(&self) -> Option<String> {
        unsafe { c_to_rust_string((*self.device).output_name) }
    }

    /// Get the size in (width_mm, height_mm) format.
    ///
    /// These values will be 0 if it's not supported.
    pub fn size(&self) -> (c_double, c_double) {
        unsafe { ((*self.device).width_mm, (*self.device).height_mm) }
    }

    /// Get the type of the device
    pub fn dev_type(&self) -> wlr_input_device_type {
        unsafe { (*self.device).type_ }
    }

    /// Get a handle to the backing input device.
    pub fn device(&self) -> Handle {
        unsafe {
            match self.dev_type() {
                WLR_INPUT_DEVICE_KEYBOARD => {
                    let keyboard_ptr = (*self.device).__bindgen_anon_1.keyboard;
                    Handle::Keyboard(keyboard::Handle::from_ptr(keyboard_ptr))
                },
                WLR_INPUT_DEVICE_POINTER => {
                    let pointer_ptr = (*self.device).__bindgen_anon_1.pointer;
                    Handle::Pointer(pointer::Handle::from_ptr(pointer_ptr))
                },
                WLR_INPUT_DEVICE_TOUCH => {
                    let touch_ptr = (*self.device).__bindgen_anon_1.touch;
                    Handle::Touch(touch::Handle::from_ptr(touch_ptr))
                },
                WLR_INPUT_DEVICE_TABLET_TOOL => {
                    let tablet_tool_ptr = (*self.device).__bindgen_anon_1.tablet;
                    Handle::TabletTool(tablet_tool::Handle::from_ptr(tablet_tool_ptr))
                },
                WLR_INPUT_DEVICE_TABLET_PAD => {
                    let tablet_pad_ptr = (*self.device).__bindgen_anon_1.tablet_pad;
                    Handle::TabletPad(tablet_pad::Handle::from_ptr(tablet_pad_ptr))
                },
            }
        }
    }

    pub(crate) unsafe fn dev_union(&self) -> wlr_input_device_pointer {
        (*self.device).__bindgen_anon_1
    }

    pub(crate) unsafe fn from_ptr(device: *mut wlr_input_device) -> Self {
        Device { device: device }
    }

    pub(crate) unsafe fn as_ptr(&self) -> *mut wlr_input_device {
        self.device
    }
}
