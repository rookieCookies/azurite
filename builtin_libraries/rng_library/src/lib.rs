use rand::{thread_rng, Rng};
use azurite_runtime::{VM, VMData, Result};

#[no_mangle]
pub extern "C" fn randi(vm: &mut VM) -> Result {
    vm.stack.set_reg(0, VMData::Integer(thread_rng().gen()));
    Result::Ok
}

#[no_mangle]
pub extern "C" fn randf(vm: &mut VM) -> Result {
    vm.stack.set_reg(0, VMData::Float(thread_rng().gen()));
    Result::Ok
}