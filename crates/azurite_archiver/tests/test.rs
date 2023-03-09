use azurite_archiver::{Packed, Data};

#[test]
fn hello() {
    let packed = Packed::new()
        .with(Data(vec![5, 3, 2]));

    let bytes = packed.clone().as_bytes();

    println!("{:#?} {:#?}", packed, bytes);
    assert_eq!(Some(packed), Packed::from_bytes(bytes.iter()));
}