use azurite_errors::SourcedDataType;
use common::SymbolIndex;

#[derive(Debug, PartialEq)]
pub struct VariableStack {
    values: Vec<(SymbolIndex, SourcedDataType)>,
}

impl VariableStack {
    pub fn new() -> Self {
        Self {
            values: Vec::with_capacity(128),
        }
    }


    pub(crate) fn find(&self, str: SymbolIndex) -> Option<SourcedDataType> {
        self.values.iter().rev().find_map(|x| if x.0 == str { Some(x.1) } else { None })
    }

    pub(crate) fn pop(&mut self, amount: usize) {
        (0..amount).for_each(|_| { self.values.pop(); });
    }

    pub(crate) fn push(&mut self, identifier: SymbolIndex, value: SourcedDataType) {
        self.values.push((identifier, value));
    }

    pub(crate) fn len(&self) -> usize {
        self.values.len()
    }
}

impl Default for VariableStack {
    fn default() -> Self {
        Self::new()
    }
}
