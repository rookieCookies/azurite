use crate::{Object, ObjectData};

#[derive(Debug, Clone, Default)]
pub struct ObjectMap {
    pub data: Vec<Object>,
    pub free: usize,
}

impl ObjectMap {
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: (0..capacity)
                .map(|index| Object::new(ObjectData::Free { next: index + 1 }))
                .collect(),
            free: 0,
        }
    }

    pub fn push(&mut self, value: Object) -> Result<usize, Object> {
        if self.free >= self.data.capacity() {
            return Err(value);
        }
        match self.data[self.free].data {
            crate::ObjectData::Free { next } => {
                self.data[self.free] = value;
                let replaced_index = self.free;
                self.free = next;
                Ok(replaced_index)
            }
            _ => panic!("can't replace a not-freed object"),
        }
    }

    pub fn remove(&mut self, index: usize) {
        self.data[index] = Object::new(ObjectData::Free { next: self.free });
        self.free = index;
    }

    pub fn get(&self, index: usize) -> Option<&Object> {
        self.data.get(index)
    }

    pub fn get_mut(&mut self, index: usize) -> Option<&mut Object> {
        self.data.get_mut(index)
    }

    pub fn len(&self) -> usize {
        self.data.len()
    }

    pub fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}
