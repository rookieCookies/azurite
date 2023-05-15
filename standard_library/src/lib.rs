use azurite_runtime::{Stack, VM};

#[no_mangle]
pub extern "C" fn print(vm: &mut VM) {
    let string = match vm.stack.reg(1) {
        azurite_runtime::VMData::Object(v) => vm.objects.get(v as usize),
        _ => unreachable!()
    };

    if let azurite_runtime::Object::String(v) = string {
        print!("{}", v)
    }
}

#[no_mangle]
pub extern "C" fn println(vm: &mut VM) {
    let string = match vm.stack.reg(1) {
        azurite_runtime::VMData::Object(v) => vm.objects.get(v as usize),
        _ => unreachable!()
    };

    if let azurite_runtime::Object::String(v) = string {
        println!("{}", v)
    }
}
