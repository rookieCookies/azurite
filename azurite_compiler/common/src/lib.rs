use azurite_errors::SourceRange;

#[derive(Debug, Clone, Copy)]
pub struct SourcedDataType {
    pub source_range: SourceRange,
    pub data_type: DataType,
}

impl SourcedDataType {
    pub fn new(source_range: SourceRange, data_type: DataType) -> Self { Self { source_range, data_type } }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DataType {
    Integer,
    Float,
    String,
    Bool,
    Empty,
    Any,
    
    Struct(SymbolIndex),
}


impl SourcedDataType {
    pub fn from(value: &SourcedData) -> Self {
        Self::new(value.source_range, DataType::from(&value.data))
    }
    
}


impl DataType {
    pub fn from(value: &Data) -> Self {
        match value {
            Data::Int(_)    => DataType::Integer,
            Data::Float(_)  => DataType::Float,
            Data::String(_) => DataType::String,
            Data::Bool(_)   => DataType::Bool,
            Data::Empty     => DataType::Empty,
        }
    }
    
}

impl DataType {
    pub fn to_string(self, symbol_table: &SymbolTable) -> String {
        match self {
            DataType::Integer   => "integer".to_string(),
            DataType::Float     => "float".to_string(),
            DataType::String    => "string".to_string(),
            DataType::Bool      => "bool".to_string(),
            DataType::Empty     => "()".to_string(),
            DataType::Any       => "any".to_string(),
            DataType::Struct(v) => symbol_table.get(v)
        }
    }
}


#[derive(Debug)]
pub struct SourcedData {
    pub source_range: SourceRange,
    pub data: Data,
}

impl SourcedData {
    pub fn new(source_range: SourceRange, data_type: Data) -> Self { Self { source_range, data: data_type } }
}


#[derive(Debug, PartialEq)]
pub enum Data {
    Int   (i64),
    Float (f64),
    String(SymbolIndex),
    Bool  (bool),

    Empty,
}

impl Data {
    pub fn to_string(&self, symbol_table: &SymbolTable) -> String {
        match self {
            Data::Int(v)    => v.to_string(),
            Data::Float(v)  => v.to_string(),
            Data::String(v) => symbol_table.get(*v),
            Data::Bool(v)   => v.to_string(),
            Data::Empty     => "()".to_string(),
        }
    }
}


#[derive(Debug)]
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
            SymbolTableValue::Combo(v1, v2) => format!("[{}<|::({}, {v2:?})]", self.get(*v1), self.get(*v2)),
        }
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


#[derive(Debug)]
enum SymbolTableValue {
    String(String),
    Combo(SymbolIndex, SymbolIndex)
}