#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataType {
    I8,
    I16,
    I32,
    I64,
    U8,
    U16,
    U32,
    U64,
    
    Float,
    String,
    Bool,
    Empty,
    Any,
    
    Struct(SymbolIndex),
}


impl DataType {
    pub const fn is_signed_integer(self) -> bool {
        matches!(self,
            | Self::I8
            | Self::I16
            | Self::I32
            | Self::I64
        )
    }

    
    pub fn from(value: &Data) -> Self {
        match value {
            Data::Float(_)  => DataType::Float,
            Data::String(_) => DataType::String,
            Data::Bool(_)   => DataType::Bool,
            Data::Empty     => DataType::Empty,
            Data::I8(_)  => DataType::I8,
            Data::I16(_) => DataType::I16,
            Data::I32(_) => DataType::I32,
            Data::I64(_) => DataType::I64,
            Data::U8(_)  => DataType::U8,
            Data::U16(_) => DataType::U16,
            Data::U32(_) => DataType::U32,
            Data::U64(_) => DataType::U64,
        }
    }
    
}

impl DataType {
    pub fn to_string(self, symbol_table: &SymbolTable) -> String {
        match self {
            DataType::I8        => "i8".to_string(),
            DataType::I16       => "i16".to_string(),
            DataType::I32       => "i32".to_string(),
            DataType::I64       => "i64".to_string(),
            DataType::U8        => "u8".to_string(),
            DataType::U16       => "u16".to_string(),
            DataType::U32       => "u32".to_string(),
            DataType::U64       => "u64".to_string(),
            DataType::Float     => "float".to_string(),
            DataType::String    => "str".to_string(),
            DataType::Bool      => "bool".to_string(),
            DataType::Empty     => "()".to_string(),
            DataType::Any       => "any".to_string(),
            DataType::Struct(v) => symbol_table.get(v)
        }
    }


    pub fn symbol_index(self, symbol_table: &mut SymbolTable) -> SymbolIndex {
        match self {
            DataType::Struct(v) => v,
            _ => symbol_table.add(self.to_string(symbol_table))
        }
    }
}


#[derive(Debug, PartialEq)]
pub enum Data {
    I8    (i8),
    I16   (i16),
    I32   (i32),
    I64   (i64),
    U8    (u8),
    U16   (u16),
    U32   (u32),
    U64   (u64),

    Float (f64),
    String(SymbolIndex),
    Bool  (bool),

    Empty,
}

impl Data {
    pub fn to_string(&self, symbol_table: &SymbolTable) -> String {
        match self {
            Data::Float(v)  => v.to_string(),
            Data::String(v) => symbol_table.get(*v),
            Data::Bool(v)   => v.to_string(),
            Data::Empty     => "()".to_string(),
            Data::I8 (v)    => v.to_string(),
            Data::I16(v)    => v.to_string(),
            Data::I32(v)    => v.to_string(),
            Data::I64(v)    => v.to_string(),
            Data::U8 (v)    => v.to_string(),
            Data::U16(v)    => v.to_string(),
            Data::U32(v)    => v.to_string(),
            Data::U64(v)    => v.to_string(),
        }
    }
}


#[derive(Debug, PartialEq)]
pub struct SymbolTable {
    vec: Vec<SymbolTableValue>,
}

impl SymbolTable {
    pub fn new() -> Self { Self { vec: vec![] } }

    pub fn add(&mut self, string: String) -> SymbolIndex {
        match self.vec.iter().enumerate().find(|x| match x.1 {
            SymbolTableValue::String(v) => v == &string,
            SymbolTableValue::Combo(_, _) => false,
        }) {
            Some(v) => SymbolIndex(v.0),
            None => {
                self.vec.push(SymbolTableValue::String(string));
        
                SymbolIndex(self.vec.len()-1)
            },
        }
    }


    pub fn add_combo(&mut self, one: SymbolIndex, two: SymbolIndex) -> SymbolIndex {
        match self.vec.iter().enumerate().find(|x| match x.1 {
            SymbolTableValue::String(_) => false,
            SymbolTableValue::Combo(v1, v2) => v1 == &one && v2 == &two,
        }) {
            Some(v) => SymbolIndex(v.0),
            None => {
                self.vec.push(SymbolTableValue::Combo(one, two));
        
                SymbolIndex(self.vec.len()-1)
            },
        }
    }


    pub fn get(&self, index: SymbolIndex) -> String {
        match &self.vec[index.0] {
            SymbolTableValue::String(v) => v.to_owned(),
            SymbolTableValue::Combo(v1, v2) => format!("{}::{}", self.get(*v1), self.get(*v2)),
        }
    }


    pub fn find_root(&self, index: SymbolIndex) -> (SymbolIndex, Option<SymbolIndex>) {
        match &self.vec[index.0] {
            SymbolTableValue::String(_) => (index, None),
            SymbolTableValue::Combo(v1, v2) => {
                match &self.vec[v1.0] {
                    SymbolTableValue::String(_) => (*v1, Some(*v2)),
                    SymbolTableValue::Combo(_, _) => self.find_root(*v1),
                }
            },
        }
    }


    pub fn find_combo(&self, v1: SymbolIndex, v2: SymbolIndex) -> SymbolIndex {
        let mock = SymbolTableValue::Combo(v1, v2);
        SymbolIndex(self.vec.iter().enumerate().find(|x| *x.1 == mock).unwrap().0)
    }


    pub fn find(&self, val: &str) -> Option<SymbolIndex> {
        for i in self.vec.iter().enumerate() {
            if let SymbolTableValue::String(v) = i.1 {
                if v.as_str() == val {
                    return Some(SymbolIndex(i.0))
                }
            }
        }

        None
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}


#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct SymbolIndex(usize);

impl SymbolIndex {
    pub const DECOY : SymbolIndex = SymbolIndex(usize::MAX);
}


#[derive(Debug, PartialEq, Eq)]
enum SymbolTableValue {
    String(String),
    Combo(SymbolIndex, SymbolIndex)
}