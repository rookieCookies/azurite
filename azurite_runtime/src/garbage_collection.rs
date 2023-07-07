use std::{sync::atomic::AtomicU64, time::Instant};

use rayon::prelude::{IntoParallelRefMutIterator, IndexedParallelIterator, ParallelIterator};

use crate::{VM, Object, object_map::{ObjectMap, ObjectData, ObjectIndex}};

impl VM<'_> {
    pub fn run_garbage_collection(&mut self) {
        self.debug.last_gc_time = std::time::SystemTime::now();
        self.debug.total_gc_count += 1;
        let instant = Instant::now();
        
        self.mark();
        self.sweep();

        let elapsed = instant.elapsed();
        self.debug.last_gc_duration = elapsed;
        
    }


    fn mark(&mut self) {
        for object in 0..self.stack.top {
            let val = self.stack.values[object];
            if val.is_object() {
                self.objects.get(val.as_object()).mark(true, &self.objects);
            }
        }

        for val in &self.constants {
            if val.is_object() {
                self.objects.get(val.as_object()).mark(true, &self.objects);
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


    pub fn memory_usage(&self) -> usize {
       self.objects.raw()
            .iter()
            .map(|obj| {
                match &obj.data {
                    ObjectData::String(v) => std::mem::size_of::<Object>() + v.capacity(),
            
                    // We don't need to add up the inner-objects as all objects are in
                    // the object map so eventually we will also add that objects size
                    ObjectData::Struct(v) => std::mem::size_of::<Object>() + std::mem::size_of_val(v.fields()),

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
        if self.liveliness_status.replace(mark_as) {
            return
        }

        match &self.data {
            ObjectData::Struct(v) => v.fields().iter().filter(|x| x.is_object()).for_each(|x| objects.get(x.as_object()).mark(mark_as, objects)),
            
            | ObjectData::String(_)
            | ObjectData::Free { .. } => (),
        }
    }
}
