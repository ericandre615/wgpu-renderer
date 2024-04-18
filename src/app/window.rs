use winit::window::Window as WinitWindow;

pub struct WindowRef<'a> {
    window: &'a WinitWindow,
    ref_count: std::cell::RefCell<i32>,
}

impl<'a> WindowRef<'a> {
    pub fn new(window: &'a WinitWindow) -> Self {
        WindowRef {
            window,
            ref_count: std::cell::RefCell::new(1),
        }
    }

    pub fn borrow(&self) -> &winit::window::Window {
        self.window
    }

    pub fn clone(&self) -> Self {
        let mut ref_count = self.ref_count.borrow_mut();
        *ref_count += 1;

        Self {
            window: self.window,
            ref_count: std::cell::RefCell::new(ref_count.clone()),
        }
    }
}
