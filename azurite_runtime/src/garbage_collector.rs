use std::mem::size_of;

use crate::{vm::VM, Object};

impl VM {
    // TODO: Improve, just kind of slapped it here I mean cmon dude
    pub fn collect_garbage(&mut self) {
        self.mark();
        self.sweep();
    }

    fn mark(&mut self) {
        for object in 0..self.stack.top {
            match self.stack.data[object] {
                crate::VMData::Object(index) => {
                    let object =
                        unsafe { &*(self.objects.get(index as usize).unwrap() as *const Object) };
                    object.mark_inner(&mut self.objects);
                }
                _ => continue,
            }
        }

        for object in &self.constants {
            match object {
                crate::VMData::Object(index) => {
                    let object = unsafe {
                        &*(self.objects.data.get_unchecked(*index as usize) as *const Object)
                    };
                    object.mark_inner(&mut self.objects);
                }
                _ => continue,
            }
        }
    }

    fn sweep(&mut self) {
        self.objects.data.retain(|obj| obj.live);
    }

    #[must_use]
    pub fn usage(&self) -> usize {
        self.objects.len() * size_of::<Object>()
    }
}
