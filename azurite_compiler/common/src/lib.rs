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
    pub fn to_string(self, symbol_table: &SymbolTable) -> &str {
        match self {
            DataType::Integer   => "integer",
            DataType::Float     => "float",
            DataType::String    => "string",
            DataType::Bool      => "bool",
            DataType::Empty     => "()",
            DataType::Any       => "any",
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
            Data::String(v) => symbol_table.get(*v).to_string(),
            Data::Bool(v)   => v.to_string(),
            Data::Empty     => "()".to_string(),
        }
    }
}


#[derive(Debug)]
pub struct SymbolTable {
    vec: Vec<String>,
}

impl SymbolTable {
    pub fn new() -> Self { Self { vec: vec![] } }

    pub fn add(&mut self, string: String) -> SymbolIndex {
        match self.vec.iter().enumerate().find(|x| x.1 == &string) {
            Some(v) => SymbolIndex(v.0),
            None => {
                self.vec.push(string);
        
                SymbolIndex(self.vec.len()-1)
            },
        }
    }


    pub fn get(&self, index: SymbolIndex) -> &str {
        self.vec[index.0].as_str()
    }
}

impl Default for SymbolTable {
    fn default() -> Self {
        Self::new()
    }
}


#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq, PartialOrd, Ord)]
pub struct SymbolIndex(usize);