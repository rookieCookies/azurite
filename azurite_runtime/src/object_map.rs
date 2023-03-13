use crate::{Object, ObjectData};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ObjectMap {
    pub data: Vec<Object>,
    pub free: usize,
}

impl ObjectMap {
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(0)
    }

    #[must_use]
    pub fn with_capacity(capacity: usize) -> Self {
        Self {
            data: (0..capacity)
                .map(|index| Object::new(ObjectData::Free { next: index + 1 }))
                .collect(),
            free: 0,
        }
    }

    /// # Errors
    /// This function will error if the `ObjectMap` is out of
    /// free objects or the `free` object the map has is not
    /// actually `ObjectData::Free`
    pub(crate) fn push(&mut self, value: Object) -> Result<usize, Object> {
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
            _ => Err(value),
        }
    }

    #[must_use]
    pub(crate) fn get(&self, index: usize) -> Option<&Object> {
        self.data.get(index)
    }

    #[must_use]
    pub(crate) fn get_mut(&mut self, index: usize) -> Option<&mut Object> {
        self.data.get_mut(index)
    }
}
