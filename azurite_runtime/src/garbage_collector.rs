use crate::{vm::VM, Object, VMData, object_map::ObjectMap, ObjectData};

impl VM {
    // TODO: Improve, just kind of slapped it here I mean cmon dude
    pub fn collect_garbage(&mut self) {
        self.mark();
        self.sweep();
    }

    fn mark(&mut self) {
        let a = unsafe { &mut *std::ptr::addr_of_mut!(self.objects) };
        self.objects.data.iter_mut().for_each(|x| x.mark_inner(false, a));

        for object in 0..self.stack.top {
            match self.stack.data[object] {
                crate::VMData::Object(index) => self.objects.get(index as usize).unwrap().mark_inner(true, &self.objects),
                _ => continue,
            };
        }

        for object in &self.constants {
            match object {
                crate::VMData::Object(index) => self.objects.get(*index as usize).unwrap().mark_inner(true, &self.objects),
                _ => continue,
            }
        }
    }

    fn sweep(&mut self) {
        self.objects.data.retain(|obj| obj.live.take());
    }

    #[must_use]
    pub fn usage(&self) -> usize {
        self.objects.data.iter().map(|obj| {
            match &obj.data {
                crate::ObjectData::String(v) => std::mem::size_of::<Object>() + v.len(),
        
                // We don't need to add up the inner-objects as all objects are in
                // the object map so eventually we will also add that objects size
                | crate::ObjectData::List(v)
                | crate::ObjectData::Struct(v) => std::mem::size_of::<Object>() + v.len() * std::mem::size_of::<VMData>(),

                // If the object is free, it is technically still occupying space
                // in the VM but that is not considered as "used" memory so it
                // would not be accurate to add it in the calculation
                crate::ObjectData::Free { .. } => 0,
            }
        }).sum()
    }
}

impl Object {
    fn mark_inner(&self, mark_as: bool, objects: &ObjectMap) {
        self.live.set(mark_as);
        match &self.data {
            ObjectData::List(v) | ObjectData::Struct(v) => v.iter().for_each(|x| {
                if let VMData::Object(value) = x {
                    objects.data.get(*value as usize).unwrap().mark_inner(mark_as, objects);
                }
            }),
            _ => (),
        }
    }
}