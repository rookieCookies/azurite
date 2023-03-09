use std::{hash::{Hash, Hasher}, collections::hash_map::DefaultHasher, slice::Iter};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Packed {
    data_table: Vec<Data>,
}

impl Packed {
    pub fn new() -> Packed {
        Self {
            data_table: vec![],
        }
    }

    pub fn with(mut self, value: impl Into<Data>) -> Self {
        self.data_table.push(value.into());
        self
    }

    pub fn as_bytes(mut self) -> Vec<u8> {
        if self.data_table.is_empty() {
            return vec![]
        }

        let size_of_lookup_table = self.data_table.len() * 8;

        let total_size : usize = 
            self.data_table.iter().map(Data::size).sum::<usize>()   // Size of the data
            + self.data_table.len()                                 // Marker for how many items there are in the look-up table
            + size_of_lookup_table                                  // Size of the lookup-table for each data
            + 8                                                     // Size of the version number
            + 8                                                     
            ;
        


        let mut bytes = Vec::with_capacity(total_size);
        
        {
            let version_number = env!("CARGO_PKG_VERSION");
            let mut hasher = DefaultHasher::new();
            version_number.hash(&mut hasher);
        
            bytes.append(&mut hasher.finish().to_le_bytes().into())
        }

        {
            let lookup_table_size : u64 = self.data_table.len().try_into().expect("unable to convert usize to u64");
            bytes.append(&mut lookup_table_size.to_le_bytes().into());
        }

        {
            for data in self.data_table.iter() {
                let size : u64 = data.size().try_into().expect("unable to convert usize to u64");
                bytes.append(&mut size.to_le_bytes().into());
            }
        }
        
        {
            for data in self.data_table.iter_mut() {
                bytes.append(&mut data.0)
            }
        }

        bytes
    }

    pub fn from_bytes(mut iterator: Iter<u8>) -> Option<Packed> {
        let _version_hash = take_u64(&mut iterator)?;               // Just there if I ever need it

        let mut lookup_table : Vec<_>;
        {
            let lookup_table_size = take_u64(&mut iterator)?;
            lookup_table = Vec::with_capacity(lookup_table_size as usize);

            for _ in 0..lookup_table_size {
                lookup_table.push(take_u64(&mut iterator)?);
            }
        }

        let mut data_table = Vec::with_capacity(lookup_table.len());
        for size in lookup_table {
            let mut data = Vec::with_capacity(size as usize);
            for _ in 0..size {
                data.push(*iterator.next()?);
            }

            data_table.push(Data(data))
        }


        Some(Packed {
            data_table,
        })
    }
}

impl From<Packed> for Vec<Data> {
    fn from(val: Packed) -> Self {
        val.data_table
    }
}

fn take_u64(iterator: &mut Iter<u8>) -> Option<u64> {
    let value = u64::from_le_bytes([
        *iterator.next()?,
        *iterator.next()?,
        *iterator.next()?,
        *iterator.next()?,
        *iterator.next()?,
        *iterator.next()?,
        *iterator.next()?,
        *iterator.next()?,
    ]);
    Some(value)
}

impl Default for Packed {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Data(pub Vec<u8>);

impl Data {
    fn size(&self) -> usize {
        self.0.len()
    }
}

