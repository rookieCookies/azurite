use rand::{thread_rng, Rng};
use azurite_runtime::{VM, VMData, Status};

#[no_mangle]
pub extern "C" fn randi(vm: &mut VM) -> Status {
    vm.stack.set_reg(0, VMData::new_i64(thread_rng().gen()));
    Status::Ok
}

#[no_mangle]
pub extern "C" fn randf(vm: &mut VM) -> Status {
    vm.stack.set_reg(0, VMData::new_float(thread_rng().gen()));
    Status::Ok
}