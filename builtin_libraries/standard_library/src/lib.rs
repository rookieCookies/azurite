use std::io::Write;

use azurite_runtime::{VM, Object, VMData, FatalError, Status, ObjectIndex};


#[no_mangle]
pub extern "C" fn _shutdown(_: &mut VM) -> Status {
    if std::io::stdout().lock().flush().is_err() {
        return Status::err("failed to flush stdout")
    }

    Status::Ok
}



#[no_mangle]
pub extern "C" fn print(vm: &mut VM) -> Status {
    let string = vm.stack.reg(1).object();

    let string = vm.objects.get(string).string();
    print!("{string}");

    Status::Ok
}


#[no_mangle]
pub extern "C" fn println(vm: &mut VM) -> Status {
    let string = vm.stack.reg(1).object();

    let string = vm.objects.get(string).string();
    println!("{string}");

    Status::Ok
}


#[no_mangle]
pub extern "C" fn read_line(vm: &mut VM) -> Status {
    let mut string = String::new();

    if std::io::stdin().read_line(&mut string).is_err() {
        return Status::err("failed to read stdin")
    }

    let temp = VMData::Object(register_string(vm, string)?);
    vm.stack.set_reg(0, temp);

    Status::Ok
}


#[no_mangle]
pub extern "C" fn exit(vm: &mut VM) -> Status {
    let exit_code = vm.stack.reg(1).integer();

    std::process::exit(exit_code as i32)
}


#[no_mangle]
pub extern "C" fn get_var(vm: &mut VM) -> Status {
    let get_value = vm.stack.reg(1).object();
    let get_value = vm.objects.get(get_value).string();

    let env_val = match std::env::var(get_value) {
        Ok(v) => v,
        Err(_) => unreachable!(),
    };

    let index = register_string(vm, env_val)?;
    vm.stack.set_reg(0, VMData::Object(index));

    Status::Ok
}


#[no_mangle]
pub extern "C" fn set_var(vm: &mut VM) -> Status {
    let set_addr = vm.stack.reg(1).object();
    let set_addr = vm.objects.get(set_addr).string();

    let set_value = vm.stack.reg(2).object();
    let set_value = vm.objects.get(set_value).string();

    std::env::set_var(set_addr, set_value);

    Status::Ok
}


#[no_mangle]
pub extern "C" fn panic(vm: &mut VM) -> Status {
    let string = vm.stack.reg(1).object();
    let string = vm.objects.get(string).string();

    Status::err(string)
}


#[no_mangle]
pub extern "C" fn int_to_str(vm: &mut VM) -> Status {
    let integer = vm.stack.reg(1).integer();

    let object = register_string(vm, integer.to_string())?;
    vm.stack.set_reg(0, VMData::Object(object));

    Status::Ok
}


#[no_mangle]
pub extern "C" fn int_to_float(vm: &mut VM) -> Status {
    let integer = vm.stack.reg(1).integer();

    vm.stack.set_reg(0, VMData::Float(integer as f64));

    Status::Ok
}


#[no_mangle]
pub extern "C" fn int_to_bool(vm: &mut VM) -> Status {
    let integer = vm.stack.reg(1).integer();

    vm.stack.set_reg(0, VMData::Bool(integer != 0));

    Status::Ok
}


#[no_mangle]
pub extern "C" fn float_to_str(vm: &mut VM) -> Status {
    let float = vm.stack.reg(1).float();

    let object = register_string(vm, float.to_string())?;
    vm.stack.set_reg(0, VMData::Object(object));

    Status::Ok
}


#[no_mangle]
pub extern "C" fn float_to_int(vm: &mut VM) -> Status {
    let float = vm.stack.reg(1).float();

    vm.stack.set_reg(0, VMData::Integer(float as i64));

    Status::Ok
}


#[no_mangle]
pub extern "C" fn float_to_bool(vm: &mut VM) -> Status {
    let float = vm.stack.reg(1).float();

    vm.stack.set_reg(0, VMData::Bool(float != 0.0));

    Status::Ok
}


#[no_mangle]
pub extern "C" fn bool_to_str(vm: &mut VM) -> Status {
    let boolean = vm.stack.reg(1).bool();

    let object = register_string(vm, boolean.to_string())?;
    vm.stack.set_reg(0, VMData::Object(object));

    Status::Ok
}


#[no_mangle]
pub extern "C" fn bool_to_int(vm: &mut VM) -> Status {
    let boolean = vm.stack.reg(1).bool();

    vm.stack.set_reg(0, VMData::Integer(if boolean { 1 } else { 0 }));

    Status::Ok
}


#[no_mangle]
pub extern "C" fn bool_to_float(vm: &mut VM) -> Status {
    let boolean = vm.stack.reg(1).bool();

    vm.stack.set_reg(0, VMData::Float(if boolean { 1.0 } else { 0.0 }));

    Status::Ok
}


#[no_mangle]
pub extern "C" fn to_string_float(vm: &mut VM) -> Status {
    let float = vm.stack.reg(1).float();

    let object = register_string(vm, float.to_string())?;
    vm.stack.set_reg(0, VMData::Object(object));

    Status::Ok
}


#[no_mangle]
pub extern "C" fn to_string_bool(vm: &mut VM) -> Status {
    let boolean = vm.stack.reg(1).bool();
    let object = register_string(vm, boolean.to_string())?;
    vm.stack.set_reg(0, VMData::Object(object));

    Status::Ok
}


#[no_mangle]
pub extern "C" fn string_append(vm: &mut VM) -> Status {
    let other_string = vm.stack.reg(2).object();
    let other_string = vm.objects.get(other_string).string().clone();
    
    let string_index = vm.stack.reg(1).object();
    let string = vm.objects.get_mut(string_index).string_mut();
    string.push_str(other_string.as_str());

    vm.stack.set_reg(0, VMData::Object(string_index));

    Status::Ok
}


#[no_mangle]
pub extern "C" fn parse_str_as_int(vm: &mut VM) -> Status {
    let string = vm.stack.reg(1).object();
    let string = vm.objects.get(string).string().trim();

    let Ok(number) = string.parse() else {
        return Status::err("failed to parse string as int");
    };

    vm.stack.set_reg(0, VMData::Integer(number));

    Status::Ok
}


fn register_string(vm: &mut VM, string: String) -> core::result::Result<ObjectIndex, FatalError> {
    vm.create_object(Object::new(string))
}