use crate::{vm::VM, Object, object_map::ObjectMap, VMData};

impl VM {
    // TODO: Improve, just kind of slapped it here I mean cmon dude
    pub fn collect_garbage(&mut self) {
        self.mark();
        self.sweep();
    }

    fn mark(&mut self) {
        let a = unsafe { &mut *std::ptr::addr_of_mut!(self.objects) };
        self.objects
            .data.iter_mut().for_each(|x| 
            unsafe { &mut *(x as *mut Object) }
            .mark_inner(false, a)
        );

        // ! UNDEFINED BEHAVIOUR
        for object in 0..self.stack.top {
            match self.stack.data[object] {
                crate::VMData::Object(index) => {
                    let object =
                        unsafe { &mut *(self.objects.get_mut(index as usize).unwrap() as *mut Object) };
                    object.mark_inner(true, &mut self.objects);
                }
                _ => continue,
            }
        }

        for object in &self.constants {
            match object {
                crate::VMData::Object(index) => {
                    let object = unsafe { &mut *(self.objects.get_mut(*index as usize).unwrap() as *mut Object) };
                    object.mark_inner(true, &mut self.objects);
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
        self.objects.data.iter().map(|obj| {
            match &obj.data {
                crate::ObjectData::String(v) => std::mem::size_of::<Object>() + v.len(),
        
                // We don't need to add up the inner-objects as all objects are in
                // the object map so eventually we will also add that objects size
                | crate::ObjectData::List(v)
                | crate::ObjectData::Struct(v) => std::mem::size_of::<Object>() + v.iter().map(|_| std::mem::size_of::<VMData>()).sum::<usize>(),

                // If the object is free, it is technically still occupying space
                // in the VM but that is not considered as "used" memory so it
                // would not be accurate to add it in the calculation
                crate::ObjectData::Free { .. } => 0,
            }
        }).sum()
    }
}
