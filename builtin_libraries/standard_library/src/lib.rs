use std::io::Write;

use azurite_runtime::{VM, Object, VMData};

#[no_mangle]
pub extern "C" fn _shutdown(_: &mut VM) {
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


#[no_mangle]
pub extern "C" fn panic(vm: &mut VM) {
    let string = vm.stack.reg(1).object();
    let string = vm.objects.get(string as usize).string();

    // TODO: Change to a runtime error obvi
    panic!("{string}");
}


#[no_mangle]
pub extern "C" fn int_to_str(vm: &mut VM) {
    let integer = vm.stack.reg(1).integer();

    let object = register_string(vm, integer.to_string());
    vm.stack.set_reg(0, VMData::Object(object as u64))
}


#[no_mangle]
pub extern "C" fn int_to_float(vm: &mut VM) {
    let integer = vm.stack.reg(1).integer();

    vm.stack.set_reg(0, VMData::Float(integer as f64))
}


#[no_mangle]
pub extern "C" fn int_to_bool(vm: &mut VM) {
    let integer = vm.stack.reg(1).integer();

    vm.stack.set_reg(0, VMData::Bool(integer != 0))
}


#[no_mangle]
pub extern "C" fn float_to_str(vm: &mut VM) {
    let float = vm.stack.reg(1).float();

    let object = register_string(vm, float.to_string());
    vm.stack.set_reg(0, VMData::Object(object as u64))
}


#[no_mangle]
pub extern "C" fn float_to_int(vm: &mut VM) {
    let float = vm.stack.reg(1).float();

    vm.stack.set_reg(0, VMData::Integer(float as i64))
}


#[no_mangle]
pub extern "C" fn float_to_bool(vm: &mut VM) {
    let float = vm.stack.reg(1).float();

    vm.stack.set_reg(0, VMData::Bool(float != 0.0))
}


#[no_mangle]
pub extern "C" fn bool_to_str(vm: &mut VM) {
    let boolean = vm.stack.reg(1).bool();

    let object = register_string(vm, boolean.to_string());
    vm.stack.set_reg(0, VMData::Object(object as u64))
}


#[no_mangle]
pub extern "C" fn bool_to_int(vm: &mut VM) {
    let boolean = vm.stack.reg(1).bool();

    vm.stack.set_reg(0, VMData::Integer(if boolean { 1 } else { 0 }))
}


#[no_mangle]
pub extern "C" fn bool_to_float(vm: &mut VM) {
    let boolean = vm.stack.reg(1).bool();

    vm.stack.set_reg(0, VMData::Float(if boolean { 1.0 } else { 0.0 }))
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



fn register_string(vm: &mut VM, string: String) -> usize {
    vm.objects.put(Object::String(string)).unwrap()
}