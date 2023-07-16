use std::{fmt::Write, rc::Rc, sync::Arc};

#[macro_use]
extern crate istd;

index_map!(GenericMap, GenericIndex, Vec<DataType>);

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
pub struct SourceRange {
    pub start: usize,
    pub end: usize,
}


impl SourceRange {
    pub fn new(start: usize, end: usize) -> Self { Self { start, end } }

    pub fn combine(start: SourceRange, end: SourceRange) -> Self {
        Self {
            start: start.start,
            end: end.end,
        }
    }
}


#[derive(Debug, Clone, PartialEq)]
pub struct SourcedDataType {
    pub source_range: SourceRange,
    pub data_type: DataType,
}

impl SourcedDataType {
    pub fn new(source_range: SourceRange, data_type: DataType) -> Self { Self { source_range, data_type } }
}


impl SourcedDataType {
    pub fn from(value: &SourcedData) -> Self {
        Self::new(value.source_range, DataType::from(&value.data))
    }
    
}


#[derive(Debug, PartialEq, Clone)]
pub struct SourcedData {
    pub source_range: SourceRange,
    pub data: Data,
}

impl SourcedData {
    pub fn new(source_range: SourceRange, data_type: Data) -> Self { Self { source_range, data: data_type } }
}



#[derive(Debug, Clone, PartialEq)]
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
    
    Struct(SymbolIndex, Arc<[SourcedDataType]>),
}


impl DataType {
    pub const fn is_signed_integer(&self) -> bool {
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
    pub fn is_obj(&self) -> bool {
        matches!(self, | DataType::String
            | DataType::Struct(_, _))
    }
    pub fn to_string(&self, symbol_table: &SymbolTable) -> String {
        match self {
            DataType::I8           => "i8".to_string(),
            DataType::I16          => "i16".to_string(),
            DataType::I32          => "i32".to_string(),
            DataType::I64          => "i64".to_string(),
            DataType::U8           => "u8".to_string(),
            DataType::U16          => "u16".to_string(),
            DataType::U32          => "u32".to_string(),
            DataType::U64          => "u64".to_string(),
            DataType::Float        => "float".to_string(),
            DataType::String       => "str".to_string(),
            DataType::Bool         => "bool".to_string(),
            DataType::Empty        => "()".to_string(),
            DataType::Any          => "any".to_string(),
            // DataType::Struct(v)    => symbol_table.get(v),
            DataType::Struct(v, generics) => {
                let v = symbol_table.get_name_without_generics(*v);
                let mut string = String::new();
                let _ = write!(string, "{}", symbol_table.get(&v));

                if !generics.is_empty() {
                    let _ = write!(string, "[");
                    for gen in generics.iter().enumerate() {
                        if gen.0 != 0 {
                            let _ = write!(string, ", ");
                        }

                        let _ = write!(string, "{}", gen.1.data_type.to_string(symbol_table));
                    }

                    let _ = write!(string, "]");
                }

                string
            }
        }
    }


    pub fn identifier(&self, symbol_table: &SymbolTable) -> String {
        match self {
            DataType::I8           => "i8".to_string(),
            DataType::I16          => "i16".to_string(),
            DataType::I32          => "i32".to_string(),
            DataType::I64          => "i64".to_string(),
            DataType::U8           => "u8".to_string(),
            DataType::U16          => "u16".to_string(),
            DataType::U32          => "u32".to_string(),
            DataType::U64          => "u64".to_string(),
            DataType::Float        => "float".to_string(),
            DataType::String       => "str".to_string(),
            DataType::Bool         => "bool".to_string(),
            DataType::Empty        => "()".to_string(),
            DataType::Any          => "any".to_string(),
            DataType::Struct(v, _) => symbol_table.get(v)
        }
        
    }


    pub fn symbol_index(&self, symbol_table: &mut SymbolTable) -> SymbolIndex {
        match self {
            DataType::Struct(v, _) => *v,
            _ => symbol_table.add(self.identifier(symbol_table))
        }
    }
}



#[derive(Debug, PartialEq, Clone)]
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
            Data::String(v) => symbol_table.get(v),
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
    pub fn new() -> Self { Self { vec: vec![SymbolTableValue::String(GENERIC_START_SYMBOL.to_string()), SymbolTableValue::String(GENERIC_END_SYMBOL.to_string())] } }

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


    pub fn get(&self, index: &SymbolIndex) -> String {
        match &self.vec[index.0] {
            SymbolTableValue::String(v) => v.to_owned(),
            SymbolTableValue::Combo(v1, v2) => format!("{}::{}", self.get(v1), self.get(v2)),
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

    pub fn add_generics(&mut self, symbol: SymbolIndex, generics: &[SourcedDataType]) -> SymbolIndex {
        if generics.is_empty() {
            return symbol
        }

        let generics_symbol = generic_declaration_suffix(self, generics);
        self.add_combo(symbol, generics_symbol)
    }


    pub fn get_name_without_generics(&self, symbol: SymbolIndex) -> SymbolIndex {
        let (mut base_name, mut left) = self.find_root(symbol);

        while let Some(l) = left {
            let (root, root_excluded) = self.find_root(l);

            if root == get_generic_args_symbol_start(self) {
                break
            }

            base_name = self.find_combo(base_name, root);
            left = root_excluded;
        }

        base_name
    }


    pub fn pretty_print(&self) {
        for i in self.vec.iter().enumerate() {
            println!("{:>w$} | {}", i.0, match i.1 {
                SymbolTableValue::String(v) => v.to_string(),
                SymbolTableValue::Combo(v, a) => format!("{v:?}, {a:?}"),
            }, w = self.vec.len().to_string().len())
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


#[derive(Debug, PartialEq, Eq)]
enum SymbolTableValue {
    String(String),
    Combo(SymbolIndex, SymbolIndex)
}


pub const GENERIC_START_SYMBOL : &str = "@<";
pub const GENERIC_END_SYMBOL : &str = ">@";


pub fn get_generic_args_symbol_start(symbol_table: &SymbolTable) -> SymbolIndex {
    symbol_table.find(GENERIC_START_SYMBOL).unwrap()
}


fn get_generic_args_symbol_end(symbol_table: &mut SymbolTable) -> SymbolIndex {
    symbol_table.find(GENERIC_END_SYMBOL).unwrap()
}


pub fn generic_declaration_suffix(symbol_table: &mut SymbolTable, generics: &[SourcedDataType]) -> SymbolIndex {
    let mut declaration_suffix = get_generic_args_symbol_start(symbol_table);
    
    for i in generics {
        let symbol = i.data_type.symbol_index(symbol_table);
        declaration_suffix = symbol_table.add_combo(declaration_suffix, symbol);
    }

    let end = get_generic_args_symbol_end(symbol_table);
    declaration_suffix = symbol_table.add_combo(declaration_suffix, end);

    declaration_suffix
}


pub fn default<T: Default>() -> T {
    T::default()
}
