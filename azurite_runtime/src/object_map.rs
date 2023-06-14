use std::cell::Cell;

use crate::VMData;

pub(crate) use self::lock::ObjectData;


#[repr(C)]
pub struct ObjectMap {
    map: Vec<Object>,
    pub(crate) free: ObjectIndex,
}



#[derive(Debug, Clone)]
pub struct Object {
    pub(crate) liveliness_status: Cell<bool>,
    pub(crate) data: ObjectData,
}


#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ObjectIndex {
    pub(crate) index: u64,
}

impl ObjectIndex {
    pub(crate) fn new(index: u64) -> Self { Self { index } }
}


pub(crate) mod lock {
    use super::{Structure, ObjectIndex};

    /// Runtime union of objects
    // TODO: Convert to an arena allocator maybe?
    #[derive(Debug, Clone)]
    #[repr(C)]
    pub enum ObjectData {
        Struct(Structure),
        String(String),

        /// Internal value to keep track
        /// of the free objects.
        Free { next: ObjectIndex },
    }
    

    impl From<Structure> for ObjectData {
        fn from(val: Structure) -> Self {
            ObjectData::Struct(val)
        }
    }


    impl From<String> for ObjectData {
        fn from(val: String) -> Self {
            ObjectData::String(val)
        }
    }
}


impl Object {
    pub fn new(data: impl Into<ObjectData>) -> Self { Self { liveliness_status: Cell::new(false), data: data.into() } }

    /// Returns a string reference
    ///
    /// # Panics
    /// - If the union type is not a string
    #[inline]
    #[must_use]
    pub fn string(&self) -> &String {
        match &self.data {
            ObjectData::String(v) => v,
            _ => unreachable!()
        }
    }

    
    /// Returns a mutable string reference
    ///
    /// # Panics
    /// - If the union type is not a string
    #[inline]
    #[must_use]
    pub fn string_mut(&mut self) -> &mut String {
        match &mut self.data {
            ObjectData::String(v) => v,
            _ => unreachable!()
        }
    }

    
    /// Returns a reference to a structure
    /// 
    /// # Panics
    /// - If the union type is not a structure
    #[inline]
    #[must_use]
    pub fn structure(&self) -> &Structure {
        match &self.data {
            ObjectData::Struct(v) => v,
            _ => unreachable!()
        }
    }

    
    /// Returns a reference to a structure
    /// 
    /// # Panics
    /// - If the union type is not a structure
    #[inline]
    #[must_use]
    pub fn structure_mut(&mut self) -> &mut Structure {
        match &mut self.data {
            ObjectData::Struct(v) => v,
            _ => unreachable!()
        }
    }
}

impl ObjectMap {
    pub(crate) fn new(space: usize) -> Self {
        Self {
            free: ObjectIndex::new(0),
            map: (0..space).map(|x| Object::new(ObjectData::Free { next: ObjectIndex::new(((x + 1) % space) as u64) })).collect(),
        }
    }


    /// Inserts an object to the object heap
    ///
    /// # Errors
    /// - If out of memory
    #[inline]
    pub(crate) fn put(&mut self, object: Object) -> Result<ObjectIndex, Object> {
        let index = self.free;
        let v = self.get_mut(self.free);
        let repl = std::mem::replace(v, object);

        match repl.data {
            ObjectData::Free { next } => {
                self.free = next;
                Ok(index)
            },

            _ => {
                let object = std::mem::replace(v, repl);
                Err(object)
            }
        }
    }


    /// Get an object from the object heap
    #[inline(always)]
    pub fn get(&self, index: ObjectIndex) -> &Object {
        &self.map[index.index as usize]
    }


    /// Get a mutable object from the object heap
    #[inline(always)]
    pub fn get_mut(&mut self, index: ObjectIndex) -> &mut Object {
        &mut self.map[index.index as usize]
    }


    #[inline]
    pub(crate) fn raw(&self) -> &[Object] {
        &self.map
    }

    
    #[inline]
    pub(crate) fn raw_mut(&mut self) -> &mut [Object] {
        &mut self.map
    }
}


#[derive(Debug, Clone)]
pub struct Structure {
    fields: Vec<VMData>,
}


impl Structure {
    pub fn new(fields: Vec<VMData>) -> Self {
        Self {
            fields,
        }
    }
    
    #[inline]
    pub fn fields(&self) -> &[VMData] {
        &self.fields
    }


    #[inline]
    pub fn fields_mut(&mut self) -> &mut [VMData] {
        &mut self.fields
    }
}

