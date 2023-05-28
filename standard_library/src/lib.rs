use std::io::Write;

use azurite_runtime::{Stack, VM, Object, VMData};

#[no_mangle]
pub extern "C" fn _shutdown(vm: &mut VM) {
    std::io::stdout().lock().flush().unwrap();
}



#[no_mangle]
pub extern "C" fn print(vm: &mut VM) {
    let string = vm.stack.reg(1).object();

    let string = vm.objects.get(string as usize).string();
    print!("{string}");
}


#[no_mangle]
pub extern "C" fn println(vm: &mut VM) {
    let string = vm.stack.reg(1).object();

    let string = vm.objects.get(string as usize).string();
    println!("{string}");
}


#[no_mangle]
pub extern "C" fn to_string_int(vm: &mut VM) {
    let integer = vm.stack.reg(1).integer();

    let object = register_string(vm, integer.to_string());
    vm.stack.set_reg(0, VMData::Object(object as u64))
}


#[no_mangle]
pub extern "C" fn to_string_float(vm: &mut VM) {
    let float = vm.stack.reg(1).float();

    let object = register_string(vm, float.to_string());
    vm.stack.set_reg(0, VMData::Object(object as u64))
}


#[no_mangle]
pub extern "C" fn to_string_bool(vm: &mut VM) {
    let boolean = vm.stack.reg(1).bool();
    let object = register_string(vm, boolean.to_string());
    vm.stack.set_reg(0, VMData::Object(object as u64))
}


#[no_mangle]
pub extern "C" fn exit(vm: &mut VM) {
    let exit_code = vm.stack.reg(1).integer();

    std::process::exit(exit_code as i32)
}


fn register_string(vm: &mut VM, string: String) -> usize {
    vm.objects.put(Object::String(string)).unwrap()
}