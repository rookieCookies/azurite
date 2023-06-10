use std::{sync::atomic::AtomicU64, time::Instant};

use rayon::prelude::{IntoParallelRefMutIterator, IndexedParallelIterator, ParallelIterator};

use crate::{VM, Object, object_map::{ObjectMap, ObjectData, ObjectIndex}, VMData};

impl VM {
    pub(crate) fn run_garbage_collection(&mut self) {
        self.mark();
        self.sweep();
    }


    fn mark(&mut self) {
        for object in 0..self.stack.top {
            match self.stack.values[object] {
                crate::VMData::Object(index) => self.objects.get(index).mark(true, &self.objects),
                _ => continue,
            };
        }

        for object in &self.constants {
            match object {
                crate::VMData::Object(index) => self.objects.get(*index).mark(true, &self.objects),
                _ => continue,
            }
        }
    }


    fn sweep(&mut self) {
        let free = AtomicU64::new(self.objects.free.index);
        self.objects.raw_mut()
            .par_iter_mut()
            .enumerate()
            .filter(|(_, object)| !matches!(object.data, ObjectData::Free { .. }))
            .filter(|(_, object)| !object.liveliness_status.replace(false))
            .for_each(|(index, object)| object.data = ObjectData::Free { next: ObjectIndex::new(free.swap(index as u64, std::sync::atomic::Ordering::Relaxed)) });

        self.objects.free = ObjectIndex::new(free.into_inner());
    }


    pub fn memory_usage(&mut self) -> usize {
       self.objects.raw()
            .iter()
            .map(|obj| {
                match &obj.data {
                    ObjectData::String(v) => std::mem::size_of::<Object>() + v.capacity(),
            
                    // We don't need to add up the inner-objects as all objects are in
                    // the object map so eventually we will also add that objects size
                    ObjectData::Struct(v) => std::mem::size_of::<Object>() + v.fields().len() * std::mem::size_of::<VMData>(),

                    // If the object is free, it is technically still occupying space
                    // in the VM but that is not considered as "used" memory so it
                    // would not be accurate to add it in the calculation
                    ObjectData::Free { .. } => 0,
                }
            })
            .sum()
    }
}


impl Object {
    fn mark(&self, mark_as: bool, objects: &ObjectMap) {
        self.liveliness_status.set(mark_as);

        match &self.data {
            ObjectData::Struct(v) => v.fields().iter().filter(|x| x.is_object()).for_each(|x| objects.get(x.object()).mark(mark_as, objects)),
            
            | ObjectData::String(_)
            | ObjectData::Free { .. } => (),
        }
    }
}
