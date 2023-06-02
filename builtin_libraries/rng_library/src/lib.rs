use rand::{thread_rng, Rng};
use azurite_runtime::{VM, VMData};

#[no_mangle]
pub extern "C" fn randi(vm: &mut VM) {
    vm.stack.set_reg(0, VMData::Integer(thread_rng().gen()))
}

#[no_mangle]
pub extern "C" fn randf(vm: &mut VM) {
    vm.stack.set_reg(0, VMData::Float(thread_rng().gen()))
}