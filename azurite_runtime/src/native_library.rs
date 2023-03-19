#![allow(clippy::unnecessary_wraps)]
use std::{env, io::Write, str::FromStr};

use rand::{thread_rng, Rng};

use crate::{
    runtime_error::RuntimeError,
    vm::{corrupt_bytecode, Code, VM},
    Object, ObjectData, VMData,
};

type NativeFunctionReturn = Result<(), RuntimeError>;
type NativeFunctionInput<'a, 'b, 'c> = (&'a mut VM, &'b mut Code<'c>);

pub static RAW_FUNCTIONS: [fn(NativeFunctionInput) -> NativeFunctionReturn; 17] = [
    error,
    collect_garbage,
    read_io,
    write_io,
    now,
    to_string,
    rand_int,
    rand_float,
    rand_range_int,
    rand_range_float,
    parse_str_float,
    parse_str_int,
    parse_str_bool,
    env_var,
    env_set_var,
    append_str,
    writeln_io,
];

fn error((vm, code): NativeFunctionInput) -> NativeFunctionReturn {
    let message_index = match vm.stack.pop() {
        VMData::Object(v) => v,
        _ => return Err(corrupt_bytecode()),
    };
    vm.stack.step();

    let message = match &vm.get_object(message_index as usize).data {
        crate::ObjectData::String(v) => v,
        _ => return Err(corrupt_bytecode()),
    };
    Err(RuntimeError::new_string(code.true_index() as u64, message.clone()))
}

fn collect_garbage((vm, _code): NativeFunctionInput) -> NativeFunctionReturn {
    vm.collect_garbage();
    Ok(())
}

fn read_io((vm, code): NativeFunctionInput) -> NativeFunctionReturn {
    let object_index = match vm.stack.pop() {
        VMData::Object(v) => v,
        _ => {
            return Err(corrupt_bytecode());
        }
    };
    vm.stack.step();

    let v = match &mut vm.objects.get_mut(object_index as usize).unwrap().data {
        crate::ObjectData::String(v) => v,
        _ => return Err(corrupt_bytecode()),
    };
    v.clear();

    #[cfg(afl)]
    return Ok(());

    std::io::stdout().flush().unwrap();
    match std::io::stdin().read_line(v) {
        Ok(_) => {}
        Err(_) => return Err(RuntimeError::new(code.true_index() as u64, "failed io read")),
    };
    if let Some('\n') = v.chars().next_back() {
        v.pop();
    }
    if let Some('\r') = v.chars().next_back() {
        v.pop();
    }
    Ok(())
}

fn write_io((vm, _code): NativeFunctionInput) -> NativeFunctionReturn {
    let message_index = match vm.stack.pop() {
        VMData::Object(v) => v,
        _ => return Err(corrupt_bytecode()),
    };
    vm.stack.step();
    let message = match &vm.get_object(message_index as usize).data {
        crate::ObjectData::String(v) => v,
        _ => return Err(corrupt_bytecode()),
    };

    #[cfg(not(afl))]
    print!("{message}");
    Ok(())
}

fn now((vm, _code): NativeFunctionInput) -> NativeFunctionReturn {
    vm.stack.push(VMData::Float(
        std::time::SystemTime::UNIX_EPOCH
            .elapsed()
            .unwrap()
            .as_secs_f64(),
    ))?;
    Ok(())
}

fn to_string((vm, _code): NativeFunctionInput) -> NativeFunctionReturn {
    let data = vm.stack.pop();

    let string = data.to_string(vm);
    let index = vm.create_object(Object::new(ObjectData::String(string)))?;
    
    vm.stack.step();
    vm.stack.push(VMData::Object(index as u64))?;
    Ok(())
}

fn rand_int((vm, _code): NativeFunctionInput) -> NativeFunctionReturn {
    vm.stack.push(VMData::Integer(thread_rng().gen()))?;
    Ok(())
}

fn rand_float((vm, _code): NativeFunctionInput) -> NativeFunctionReturn {
    vm.stack.push(VMData::Float(thread_rng().gen()))?;
    Ok(())
}

fn rand_range_int((vm, _code): NativeFunctionInput) -> NativeFunctionReturn {
    let max = match vm.stack.pop() {
        VMData::Integer(v) => v,
        _ => return Err(corrupt_bytecode()),
    };

    let min = match vm.stack.pop() {
        VMData::Integer(v) => v,
        _ => return Err(corrupt_bytecode()),
    };

    vm.stack.step();
    vm.stack.step();

    vm.stack
        .push(VMData::Integer(thread_rng().gen_range(min..max)))?;
    
    Ok(())
}

fn rand_range_float((vm, _code): NativeFunctionInput) -> NativeFunctionReturn {
    let max = match vm.stack.pop() {
        VMData::Float(v) => v,
        _ => return Err(corrupt_bytecode()),
    };

    let min = match vm.stack.pop() {
        VMData::Float(v) => v,
        _ => return Err(corrupt_bytecode()),
    };

    vm.stack
        .push(VMData::Float(thread_rng().gen_range(min..max)))?;
    
    vm.stack.step();
    vm.stack.step();
    Ok(())
}

fn parse_str_float((vm, code): NativeFunctionInput) -> NativeFunctionReturn {
    let vmdata = VMData::Float(parse(vm, code)?);
    vm.stack.push(vmdata)?;
    vm.stack.step();
    Ok(())
}

fn parse_str_int((vm, code): NativeFunctionInput) -> NativeFunctionReturn {
    let vmdata = VMData::Integer(parse(vm, code)?);
    vm.stack.step();
    vm.stack.push(vmdata)?;
    Ok(())
}

fn parse_str_bool((vm, code): NativeFunctionInput) -> NativeFunctionReturn {
    let vmdata = VMData::Bool(parse(vm, code)?);
    vm.stack.push(vmdata)?;
    vm.stack.step();
    Ok(())
}

fn parse<T: FromStr>(vm: &mut VM, code: &Code) -> Result<T, RuntimeError> {
    let string_index = match vm.stack.pop() {
        VMData::Object(v) => v,
        _ => return Err(corrupt_bytecode()),
    };
    let string = match &vm.get_object(string_index as usize).data {
        crate::ObjectData::String(v) => v,
        _ => return Err(corrupt_bytecode()),
    };
    match string.parse::<T>() {
        Ok(v) => Ok(v),
        Err(_) => Err(RuntimeError::new(
            code.true_index() as u64,
            "failed to parse value",
        )),
    }
}

fn env_var((vm, code): NativeFunctionInput) -> NativeFunctionReturn {
    let identifier_index = match vm.stack.pop() {
        VMData::Object(v) => v,
        _ => return Err(corrupt_bytecode()),
    };
    let identifier = match &vm.get_object(identifier_index as usize).data {
        crate::ObjectData::String(v) => v,
        _ => return Err(corrupt_bytecode()),
    };

    match env::var(identifier) {
        Ok(v) => {
            let index = vm.create_object(Object::new(ObjectData::String(v)))?;
            vm.stack.push(VMData::Object(index as u64))?;
            Ok(())
        }
        Err(_) => Err(RuntimeError::new(
            code.true_index() as u64,
            "environment variable {identifier} doesn't exist",
        )),
    }
}

fn env_set_var((vm, _code): NativeFunctionInput) -> NativeFunctionReturn {
    let value_index = match vm.stack.pop() {
        VMData::Object(v) => v,
        _ => return Err(corrupt_bytecode()),
    };
    let value = match &vm.get_object(value_index as usize).data {
        crate::ObjectData::String(v) => v.clone(),
        _ => return Err(corrupt_bytecode()),
    };

    let identifier_index = match vm.stack.pop() {
        VMData::Object(v) => v,
        _ => return Err(corrupt_bytecode()),
    };
    let identifier = match &vm.get_object(identifier_index as usize).data {
        crate::ObjectData::String(v) => v,
        _ => return Err(corrupt_bytecode()),
    };

    env::set_var(identifier, value);
    Ok(())
}

fn append_str((vm, _code): NativeFunctionInput) -> NativeFunctionReturn {
    let other_index = match vm.stack.pop() {
        VMData::Object(v) => v,
        _ => return Err(corrupt_bytecode()),
    };
    let other = match &vm.get_object(other_index as usize).data {
        crate::ObjectData::String(v) => v.clone(),
        _ => return Err(corrupt_bytecode()),
    };


    let self_index = match vm.stack.pop() {
        VMData::Object(v) => v,
        _ => return Err(corrupt_bytecode()),
    };

    
    vm.stack.step();
    vm.stack.step();
    vm.stack.swap_top_with(vm.stack.top - 2);
    vm.stack.step();

    match &mut vm.objects.data.get_mut(self_index as usize).unwrap().data {
        crate::ObjectData::String(v) => v.push_str(&other),
        _ => return Err(corrupt_bytecode()),
    };
    
    Ok(())
}

fn writeln_io((vm, _code): NativeFunctionInput) -> NativeFunctionReturn {
    let message_index = match vm.stack.pop() {
        VMData::Object(v) => v,
        _ => return Err(corrupt_bytecode()),
    };
    vm.stack.step();
    let message = match &vm.get_object(message_index as usize).data {
        crate::ObjectData::String(v) => v,
        _ => return Err(corrupt_bytecode()),
    };

    #[cfg(not(afl))]
    println!("{message}");
    Ok(())
}
