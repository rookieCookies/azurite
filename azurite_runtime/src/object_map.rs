use crate::{VMData, FatalError};

#[derive(Debug)]
#[repr(C)]
pub struct ObjectMap {
    map: Vec<Object>,
    free: usize,
}

/// Runtime union of objects
// TODO: Convert to an arena allocator maybe?
#[derive(Debug, Clone)]
#[repr(C)]
pub enum Object {
    Struct(Vec<VMData>),
    String(String),

    /// Internal value to keep track
    /// of the free objects.
    Free { next: usize },
}


impl Object {
    /// Consumes the union value and returns a string
    ///
    /// # Panics
    /// - If the union type is not a string
    #[inline]
    #[must_use]
    pub fn string(&self) -> &String {
        match self {
            Object::String(v) => v,
            _ => unreachable!()
        }
    }

    /// Consumes the union value and returns a structure vec
    /// 
    /// # Safety
    /// The user must ensure to correctly use the structure
    /// and it's fields in order.
    ///
    /// # Panics
    /// - If the union type is not a structure
    #[inline]
    #[must_use]
    pub fn structure(&self) -> &Vec<VMData> {
        match self {
            Object::Struct(v) => v,
            _ => unreachable!()
        }
    }
}

impl ObjectMap {
    pub(crate) fn new(space: usize) -> Self {
        Self {
            free: 0,
            map: (0..space).map(|x| Object::Free { next: (x + 1) % space }).collect(),
        }
    }


    /// Inserts an object to the object heap
    ///
    /// # Errors
    /// - If out of memory
    pub fn put(&mut self, object: Object) -> Result<usize, FatalError> {
        let index = self.free;
        let v = self.map.get_mut(self.free).unwrap();
        let repl = std::mem::replace(v, object);

        match repl {
            Object::Free { next } => {
                self.free = next;
                Ok(index)
            },

            _ => Err(FatalError::new(String::from("out of memory")))
        }
    }


    /// Get an object from the object heap
    pub fn get(&self, index: usize) -> &Object {
        &self.map[index]
    }


    /// Get a mutable object from the object heap
    pub fn get_mut(&mut self, index: usize) -> &mut Object {
        &mut self.map[index]
    }
}
