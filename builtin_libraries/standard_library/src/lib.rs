use std::io::Write;

use azurite_runtime::{VM, Object, VMData, FatalError, Result};

#[no_mangle]
pub extern "C" fn _shutdown(_: &mut VM) -> Result {
    if std::io::stdout().lock().flush().is_err() {
        return Result::err("failed to flush stdout")
    }

    Result::Ok
}



#[no_mangle]
pub extern "C" fn print(vm: &mut VM) -> Result {
    let string = vm.stack.reg(1).object();

    let string = vm.objects.get(string as usize).string();
    print!("{string}");

    Result::Ok
}


#[no_mangle]
pub extern "C" fn println(vm: &mut VM) -> Result {
    let string = vm.stack.reg(1).object();

    let string = vm.objects.get(string as usize).string();
    println!("{string}");

    Result::Ok
}


#[no_mangle]
pub extern "C" fn read_line(vm: &mut VM) -> Result {
    let mut string = String::new();

    if std::io::stdin().read_line(&mut string).is_err() {
        return Result::err("failed to read stdin")
    }

    let temp = VMData::Object(register_string(vm, string)? as u64);
    vm.stack.set_reg(0, temp);

    Result::Ok
}


#[no_mangle]
pub extern "C" fn exit(vm: &mut VM) -> Result {
    let exit_code = vm.stack.reg(1).integer();

    std::process::exit(exit_code as i32)
}


#[no_mangle]
pub extern "C" fn get_var(vm: &mut VM) -> Result {
    let get_value = vm.stack.reg(1).object();
    let get_value = vm.objects.get(get_value as usize).string();

    let env_val = match std::env::var(get_value) {
        Ok(v) => v,
        Err(_) => unreachable!(),
    };

    let index = register_string(vm, env_val)?;
    vm.stack.set_reg(0, VMData::Object(index as u64));

    Result::Ok
}


#[no_mangle]
pub extern "C" fn set_var(vm: &mut VM) -> Result {
    let set_addr = vm.stack.reg(1).object();
    let set_addr = vm.objects.get(set_addr as usize).string();

    let set_value = vm.stack.reg(2).object();
    let set_value = vm.objects.get(set_value as usize).string();

    std::env::set_var(set_addr, set_value);

    Result::Ok
}


#[no_mangle]
pub extern "C" fn panic(vm: &mut VM) -> Result {
    let string = vm.stack.reg(1).object();
    let string = vm.objects.get(string as usize).string();

    Result::err(string)
}


#[no_mangle]
pub extern "C" fn int_to_str(vm: &mut VM) -> Result {
    let integer = vm.stack.reg(1).integer();

    let object = register_string(vm, integer.to_string())?;
    vm.stack.set_reg(0, VMData::Object(object as u64));

    Result::Ok
}


#[no_mangle]
pub extern "C" fn int_to_float(vm: &mut VM) -> Result {
    let integer = vm.stack.reg(1).integer();

    vm.stack.set_reg(0, VMData::Float(integer as f64));

    Result::Ok
}


#[no_mangle]
pub extern "C" fn int_to_bool(vm: &mut VM) -> Result {
    let integer = vm.stack.reg(1).integer();

    vm.stack.set_reg(0, VMData::Bool(integer != 0));

    Result::Ok
}


#[no_mangle]
pub extern "C" fn float_to_str(vm: &mut VM) -> Result {
    let float = vm.stack.reg(1).float();

    let object = register_string(vm, float.to_string())?;
    vm.stack.set_reg(0, VMData::Object(object as u64));

    Result::Ok
}


#[no_mangle]
pub extern "C" fn float_to_int(vm: &mut VM) -> Result {
    let float = vm.stack.reg(1).float();

    vm.stack.set_reg(0, VMData::Integer(float as i64));

    Result::Ok
}


#[no_mangle]
pub extern "C" fn float_to_bool(vm: &mut VM) -> Result {
    let float = vm.stack.reg(1).float();

    vm.stack.set_reg(0, VMData::Bool(float != 0.0));

    Result::Ok
}


#[no_mangle]
pub extern "C" fn bool_to_str(vm: &mut VM) -> Result {
    let boolean = vm.stack.reg(1).bool();

    let object = register_string(vm, boolean.to_string())?;
    vm.stack.set_reg(0, VMData::Object(object as u64));

    Result::Ok
}


#[no_mangle]
pub extern "C" fn bool_to_int(vm: &mut VM) -> Result {
    let boolean = vm.stack.reg(1).bool();

    vm.stack.set_reg(0, VMData::Integer(if boolean { 1 } else { 0 }));

    Result::Ok
}


#[no_mangle]
pub extern "C" fn bool_to_float(vm: &mut VM) -> Result {
    let boolean = vm.stack.reg(1).bool();

    vm.stack.set_reg(0, VMData::Float(if boolean { 1.0 } else { 0.0 }));

    Result::Ok
}


#[no_mangle]
pub extern "C" fn to_string_float(vm: &mut VM) -> Result {
    let float = vm.stack.reg(1).float();

    let object = register_string(vm, float.to_string())?;
    vm.stack.set_reg(0, VMData::Object(object as u64));

    Result::Ok
}


#[no_mangle]
pub extern "C" fn to_string_bool(vm: &mut VM) -> Result {
    let boolean = vm.stack.reg(1).bool();
    let object = register_string(vm, boolean.to_string())?;
    vm.stack.set_reg(0, VMData::Object(object as u64));

    Result::Ok
}


#[no_mangle]
pub extern "C" fn string_append(vm: &mut VM) -> Result {
    let other_string = vm.stack.reg(2).object();
    let other_string = vm.objects.get(other_string as usize).string().clone();
    
    let string_index = vm.stack.reg(1).object();
    let string = vm.objects.get_mut(string_index as usize).string_mut();
    string.push_str(other_string.as_str());

    vm.stack.set_reg(0, VMData::Object(string_index));

    Result::Ok
}


#[no_mangle]
pub extern "C" fn parse_str_as_int(vm: &mut VM) -> Result {
    let string = vm.stack.reg(1).object();
    let string = vm.objects.get(string as usize).string().trim();

    let Ok(number) = string.parse() else {
        return Result::err("failed to parse string as int");
    };

    vm.stack.set_reg(0, VMData::Integer(number));

    Result::Ok
}


fn register_string(vm: &mut VM, string: String) -> core::result::Result<usize, FatalError> {
    vm.objects.put(Object::String(string))
}