use azurite_archiver::{Packed, Data};

#[test]
fn basic_test() {
    let packed = Packed::new()
        .with(Data(vec![5, 3, 2]));

    let bytes = packed.clone().as_bytes();
    dbg!(&bytes);

    assert_eq!(Some(packed), Packed::from_bytes(&bytes));
}

#[test]
fn empty_archive() {
    let packed = Packed::new();

    let bytes = packed.clone().as_bytes();
    assert_eq!(Some(packed), Packed::from_bytes(&bytes));
}

#[test]
fn from_packed_to_vec_of_data() {
    let data = Data(vec![0, 1, 2, 3]);

    let vec = vec![data.clone()];

    let packed = Packed::new()
        .with(data);

    let packed_vec : Vec<_> = packed.into();

    assert_eq!(packed_vec, vec);
}

#[test]
fn from_vec_of_data_to_packed() {
    let data = Data(vec![0, 1, 2, 3]);

    let vec = vec![data.clone()];
    let packed = Packed::new()
        .with(data);

    let packed_created = Packed::from(vec);

    assert_eq!(packed, packed_created);
}

#[test]
fn default() {
    assert_eq!(Packed::new(), Packed::default())
}