use std::collections::HashMap;

use crate::SourcedDataType;

#[derive(Debug, PartialEq)]
pub struct GenericMap {
    map: Vec<Vec<SourcedDataType>>,
}


#[derive(PartialEq, Eq, PartialOrd, Ord, Clone, Copy, Default, Debug, Hash)]
pub struct GenericIndex(usize);


impl GenericMap {
    pub fn new() -> Self { Self { map: vec![] } }

    pub fn push(&mut self, generic: Vec<SourcedDataType>) -> GenericIndex {
        self.map.push(generic);
        GenericIndex(self.map.len())
    }


    pub fn get(&self, index: &GenericIndex) -> &Vec<SourcedDataType> {
        &self.map[index.0]
    }
}