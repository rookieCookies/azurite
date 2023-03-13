use std::{sync::atomic::AtomicUsize};

use rayon::prelude::{ParallelIterator, IntoParallelRefMutIterator, IndexedParallelIterator};

use crate::{vm::VM, Object, VMData, object_map::ObjectMap, ObjectData};

impl VM {
    pub fn collect_garbage(&mut self) {
        self.mark();
        self.sweep();
    }

    fn mark(&mut self) {
        for object in 0..self.stack.top {
            match self.stack.data[object] {
                crate::VMData::Object(index) => self.objects.data[index as usize].mark(true, &self.objects),
                _ => continue,
            };
        }

        for object in &self.constants {
            match object {
                crate::VMData::Object(index) => self.objects.data[*index as usize].mark(true, &self.objects),
                _ => continue,
            }
        }
    }

    fn sweep(&mut self) {
        let free = AtomicUsize::new(self.objects.free);
        self.objects.data
            .par_iter_mut()
            .enumerate()
            .filter(|(_, object)| !matches!(object.data, ObjectData::Free { .. }))
            .filter(|(_, object)| !object.live.replace(false))
            .for_each(|(index, object)| object.data = ObjectData::Free { next: free.swap(index, std::sync::atomic::Ordering::Relaxed) });

        self.objects.free = free.into_inner();
    }

    #[must_use]
    pub fn usage(&self) -> usize {
        self.objects.data
            .iter()
            .map(|obj| {
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
            })
            .sum()
    }
}

impl Object {
    pub(crate) fn mark(&self, mark_as: bool, objects: &ObjectMap) {
        self.live.set(mark_as);
        match &self.data {
            ObjectData::List(v) | ObjectData::Struct(v) => v.iter().for_each(|x| {
                if let VMData::Object(value) = x {
                    objects.data[*value as usize].mark(mark_as, objects);
                }
            }),
            _ => (),
        }
    }
}
