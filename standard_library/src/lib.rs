use std::io::Write;

use azurite_runtime::{Stack, VM, Object, VMData};

#[no_mangle]
pub extern "C" fn _shutdown(vm: &mut VM) {
    std::io::stdout().lock().flush().unwrap();
}



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


#[no_mangle]
pub extern "C" fn to_string_int(vm: &mut VM) {
    let string = match vm.stack.reg(1) {
        azurite_runtime::VMData::Integer(v) => v,
        _ => unreachable!()
    };

    let object = register_string(vm, string.to_string());
    vm.stack.set_reg(0, VMData::Object(object as u64))
}


#[no_mangle]
pub extern "C" fn to_string_float(vm: &mut VM) {
    let string = match vm.stack.reg(1) {
        azurite_runtime::VMData::Float(v) => v,
        _ => unreachable!()
    };

    let object = register_string(vm, string.to_string());
    vm.stack.set_reg(0, VMData::Object(object as u64))
}


#[no_mangle]
pub extern "C" fn to_string_bool(vm: &mut VM) {
    let string = match vm.stack.reg(1) {
        azurite_runtime::VMData::Bool(v) => v,
        _ => unreachable!()
    };

    let object = register_string(vm, string.to_string());
    vm.stack.set_reg(0, VMData::Object(object as u64))
}


#[no_mangle]
pub extern "C" fn fib_rec(vm: &mut VM) {
    let float = match vm.stack.reg(1) {
        azurite_runtime::VMData::Float(v) => v,
        _ => unreachable!()
    };

    let value = fib_recursive(float);
    vm.stack.set_reg(0, VMData::Float(value))
}



fn fib_recursive(n: f64) -> f64 {
    if n < 2.0 {
        return n
    }
    fib_recursive(n - 1.0) + fib_recursive(n - 2.0)
}


fn register_string(vm: &mut VM, string: String) -> usize {
    vm.objects.put(Object::String(string)).unwrap()
}