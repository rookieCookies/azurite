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


#[no_mangle]
pub extern "C" fn get_var(vm: &mut VM) {
    let get_value = vm.stack.reg(1).object();
    let get_value = vm.objects.get(get_value as usize).string();

    let env_val = match std::env::var(get_value) {
        Ok(v) => v,
        Err(_) => unreachable!(),
    };

    let index = register_string(vm, env_val);
    vm.stack.set_reg(0, VMData::Object(index as u64))
}


#[no_mangle]
pub extern "C" fn set_var(vm: &mut VM) {
    let set_addr = vm.stack.reg(1).object();
    let set_addr = vm.objects.get(set_addr as usize).string();

    let set_value = vm.stack.reg(2).object();
    let set_value = vm.objects.get(set_value as usize).string();

    std::env::set_var(set_addr, set_value);
}


fn register_string(vm: &mut VM, string: String) -> usize {
    vm.objects.put(Object::String(string)).unwrap()
}