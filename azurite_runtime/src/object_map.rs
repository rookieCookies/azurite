use crate::VMData;

#[derive(Debug)]
pub struct ObjectMap {
    map: Vec<Object>,
    free: usize,
}

#[derive(Debug, Clone)]
pub enum Object {
    Struct(Vec<VMData>),
    String(String),

    Free { next: usize },
}

impl ObjectMap {
    pub fn new() -> Self {
        Self {
            free: 0,
            map: (0..128).map(|x| Object::Free { next: (x + 1) % 128 }).collect(),
        }
    }


    pub fn put(&mut self, object: Object) -> Result<usize, String> {
        let index = self.free;
        let v = self.map.get_mut(self.free).unwrap();
        let repl = std::mem::replace(v, object);

        match repl {
            Object::Free { next } => {
                self.free = next;
                Ok(index)
            },

            _ => Err(String::from("out of memory"))
        }
    }

    
    pub fn get(&self, index: usize) -> &Object {
        &self.map[index]
    }

    
    pub fn get_mut(&mut self, index: usize) -> &mut Object {
        &mut self.map[index]
    }
}

impl Default for ObjectMap {
    fn default() -> Self {
        Self::new()
    }
}