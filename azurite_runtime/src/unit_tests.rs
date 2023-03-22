mod object_tests {

    #[test]
    fn new_object() {
        use std::cell::Cell;
        use crate::{Object, ObjectData};

        let object_data = ObjectData::String(String::new());
        let object = Object::new(object_data.clone());

        assert_eq!(object, Object { live: Cell::new(false), data: object_data });
    }

    
    #[test]
    fn new_object_map_empty() {
        use crate::object_map::ObjectMap;

        let object_map = ObjectMap::new();

        assert_eq!(object_map, ObjectMap { data: vec![], free: 0 });
    }

    
    #[test]
    fn new_object_map_with_capacity() {
        use crate::{Object, ObjectData, object_map::ObjectMap};

        let object_map = ObjectMap::with_capacity(4);

        assert_eq!(object_map, ObjectMap { data: vec![
            Object::new(ObjectData::Free { next: 1 }),
            Object::new(ObjectData::Free { next: 2 }),
            Object::new(ObjectData::Free { next: 3 }),
            Object::new(ObjectData::Free { next: 4 }),
            ], free: 0 }
        );
    }


    #[test]
    fn object_map_push() {
        use crate::{object_map::ObjectMap, Object, ObjectData};

        let mut object_map = ObjectMap::with_capacity(1);

        let object = Object::new(ObjectData::String(String::new()));

        assert!(object_map.push(object.clone()).is_ok());
        assert_eq!(object_map.free, 1);
        assert_eq!(object_map.data, vec![object]);
    }


    #[test]
    fn object_map_push_empty() {
        use crate::{object_map::ObjectMap, Object, ObjectData};

        let mut object_map = ObjectMap::with_capacity(0);

        let object = Object::new(ObjectData::String(String::new()));

        assert!(object_map.push(object).is_err());
    }

    
    #[test]
    fn object_map_push_on_free() {
        use crate::{object_map::ObjectMap, Object, ObjectData};
        
        let mut object_map = ObjectMap::with_capacity(1);

        let object = Object::new(ObjectData::String(String::new()));
        object_map.push(object.clone()).unwrap();

        object_map.free = 0; // Why would anyone ever do this, but guess gotta test it just in case I am that stupid

        assert!(object_map.push(object).is_err());
    }


    #[test]
    fn object_map_get() {
        use crate::{object_map::ObjectMap, Object, ObjectData};

        let mut object_map = ObjectMap::with_capacity(1);
        let object = Object::new(ObjectData::String(String::new()));
        
        object_map.push(object.clone()).unwrap();

        assert_eq!(object_map.get(0), Some(&object));
    }


    #[test]
    fn object_map_get_mut() {
        use crate::{object_map::ObjectMap, Object, ObjectData};

        let mut object_map = ObjectMap::with_capacity(1);
        let mut object = Object::new(ObjectData::String(String::new()));
        
        object_map.push(object.clone()).unwrap();

        assert_eq!(object_map.get_mut(0), Some(&mut object));
    }

}

mod garbage_collector_tests {
    #[test]
    fn mark_object_non_list() {
        use crate::{Object, ObjectData, object_map::ObjectMap};

        let object = Object::new(ObjectData::String(String::new()));
        let object_map = ObjectMap::new();

        object.mark(true, &object_map);

        assert!(object.live.take());
    }

    
    #[test]
    fn mark_object_list() {
        use crate::{Object, ObjectData, VMData, object_map::ObjectMap};

        let mut object_map = ObjectMap::with_capacity(8);
        let object = Object::new(ObjectData::List(vec![
            VMData::Integer(0), 
            VMData::Object(object_map.push(Object::new(ObjectData::String(String::new()))).unwrap() as u64)
            ]
        ));

        object.mark(true, &object_map);

        assert!(object.live.take());
        assert!(match object.data {
            ObjectData::List(list) => {
                match list.get(1) {
                    Some(VMData::Object(object_index)) => {
                        object_map.get(*object_index as usize).unwrap().live.take()
                    }
                    _ => false,
                }
            }
            _ => false,
        });
    }


    #[test]
    fn usage() {
        use crate::{Object, ObjectData, VMData, vm::VM};
        let mut vm = VM::new().unwrap();

        vm.create_object(Object::new(ObjectData::String("Hello!".to_string()))).unwrap(); // 40 from sizeof Object, 6 from String size = 46
        vm.create_object(Object::new(ObjectData::List(vec![
            VMData::Integer(0), 
            VMData::Bool(true),
        ]))).unwrap(); // 40 from sizeof Object, 16 * 2 from sizeof VMData = 40 + 32 = 72

        assert_eq!(vm.usage(), 118);
    }
}

mod runtime_error_test {
    #[test]
    fn load_linetable() {
        use crate::runtime_error;
        
        let mut bytes = vec![];

        bytes.append(&mut 2u32.to_le_bytes().into());
        bytes.append(&mut 5u32.to_le_bytes().into());
        
        bytes.append(&mut 4u32.to_le_bytes().into());
        bytes.append(&mut 16u32.to_le_bytes().into());

        let data = runtime_error::load_linetable(bytes);

        assert_eq!(data, vec![2, 5, 4, 16]);
    }

    
    #[test]
    fn load_linetable_empty() {
        use crate::runtime_error;

        let bytes = vec![];

        let data = runtime_error::load_linetable(bytes);

        assert_eq!(data, vec![]);
    }
}

mod stack {
    #[test]
    fn push() {
        use crate::{vm::Stack, VMData};
        
        let mut stack = Stack::new();

        stack.push(VMData::Integer(0)).unwrap();

        assert_eq!(stack.data[0], VMData::Integer(0));
        assert_eq!(stack.top, 1);
    }

    
    #[test]
    fn push_overflow() {
        use crate::{vm::Stack, VMData};
        
        let mut stack = Stack::new();

        for _ in 0..512 {
            stack.push(VMData::Integer(0)).unwrap();
        }

        assert!(stack.push(VMData::Integer(0)).is_err());
    }

    
    #[test]
    fn step() {
        use crate::vm::Stack;
        
        let mut stack = Stack::new();

        stack.step();

        assert_eq!(stack.top, 1);

        stack.step_back();

        assert_eq!(stack.top, 0);
    }
}


macro_rules! generate_equality_bytecode_test {
    ($variant: ident, $expected: expr) => {
        #[allow(non_snake_case)]
        mod $variant {

            #[test]
            fn $variant() {
                use azurite_common::Bytecode;
                use crate::{vm::VM, VMData};

                use std::cell::RefCell;
    
                let mut vm = VM::new().unwrap();
    
                // Integer
                vm.stack.push(VMData::Integer(0)).unwrap();
                vm.stack.push(VMData::Integer(0)).unwrap();
    
                assert!(vm.run(&RefCell::new(vec![]), &[Bytecode::$variant as u8, Bytecode::Return as u8]).is_ok());
                assert!(vm.stack.pop() == VMData::Bool($expected));
    
    
                // Float
                vm.stack.push(VMData::Float(0.0)).unwrap();
                vm.stack.push(VMData::Float(0.0)).unwrap();
    
                assert!(vm.run(&RefCell::new(vec![]), &[Bytecode::$variant as u8, Bytecode::Return as u8]).is_ok());
                assert!(vm.stack.pop() == VMData::Bool($expected));
    
    
                // Bool
                vm.stack.push(VMData::Bool(false)).unwrap();
                vm.stack.push(VMData::Bool(false)).unwrap();
    
                assert!(vm.run(&RefCell::new(vec![]), &[Bytecode::$variant as u8, Bytecode::Return as u8]).is_ok());
                assert!(vm.stack.pop() == VMData::Bool($expected));
            }
            
            
            #[test]
            fn object() {
                use azurite_common::Bytecode;
                use crate::{vm::VM, Object, ObjectData, VMData};

                use std::cell::RefCell;
                
                let mut vm = VM::new().unwrap();
    
                vm.create_object(Object::new(ObjectData::String(String::new()))).unwrap();
                vm.create_object(Object::new(ObjectData::String(String::new()))).unwrap();
                
                vm.stack.push(VMData::Object(0)).unwrap();
                vm.stack.push(VMData::Object(1)).unwrap();
    
                assert!(vm.run(&RefCell::new(vec![]), &[Bytecode::$variant as u8, Bytecode::Return as u8]).is_ok());
                assert!(vm.stack.pop() == VMData::Bool($expected));
            }
    
            
            #[test]
            #[should_panic]
            fn stack_too_short() {
                use azurite_common::Bytecode;
                use crate::{vm::VM, VMData};
                
                use std::cell::RefCell;
                
                let mut vm = VM::new().unwrap();
    
                vm.stack.push(VMData::Bool(false)).unwrap();
    
                // Should panic
                vm.run(&RefCell::new(vec![]), &[Bytecode::$variant as u8, Bytecode::Return as u8]).unwrap();
            }
    
            
            #[test]
            fn stack_invalid_type() {
                use azurite_common::Bytecode;
                use crate::{vm::VM, VMData};
                
                use std::cell::RefCell;
                
                let mut vm = VM::new().unwrap();
    
                vm.stack.push(VMData::Bool(false)).unwrap();
                vm.stack.push(VMData::Integer(1)).unwrap();
    
                assert!(vm.run(&RefCell::new(vec![]), &[Bytecode::$variant as u8, Bytecode::Return as u8]).is_err());
            }
        }
    }
}

macro_rules! generate_order_bytecode_test {
    ($variant: ident, $expected: expr) => {
        #[allow(non_snake_case)]
        mod $variant {
            #[test]
            fn $variant() {
                use azurite_common::Bytecode;
                use crate::{vm::VM, VMData};
                
                use std::cell::RefCell;

                let mut vm = VM::new().unwrap();

                // Integer
                vm.stack.push(VMData::Integer(1)).unwrap();
                vm.stack.push(VMData::Integer(0)).unwrap();

                assert!(vm.run(&RefCell::new(vec![]), &[Bytecode::$variant as u8, Bytecode::Return as u8]).is_ok());
                assert!(vm.stack.pop() == VMData::Bool($expected));

                // Float
                vm.stack.push(VMData::Float(1.0)).unwrap();
                vm.stack.push(VMData::Float(0.0)).unwrap();

                assert!(vm.run(&RefCell::new(vec![]), &[Bytecode::$variant as u8, Bytecode::Return as u8]).is_ok());
                assert!(vm.stack.pop() == VMData::Bool($expected));
            }

            #[test]
            #[should_panic]
            fn stack_too_short() {
                use azurite_common::Bytecode;
                use crate::{vm::VM, VMData};
                
                use std::cell::RefCell;
                
                let mut vm = VM::new().unwrap();

                vm.stack.push(VMData::Integer(0)).unwrap();

                // Should panic
                vm.run(&RefCell::new(vec![]), &[Bytecode::$variant as u8, Bytecode::Return as u8]).unwrap();
            }

            #[test]
            fn stack_invalid_type() {
                use azurite_common::Bytecode;
                use crate::{vm::VM, VMData};
                
                use std::cell::RefCell;
                
                let mut vm = VM::new().unwrap();

                vm.stack.push(VMData::Bool(false)).unwrap();
                vm.stack.push(VMData::Integer(1)).unwrap();

                assert!(vm.run(&RefCell::new(vec![]), &[Bytecode::$variant as u8, Bytecode::Return as u8]).is_err());
            }
        }
    };
}

mod vm_runtime {
    
    generate_equality_bytecode_test!(EqualsTo, true);
    generate_equality_bytecode_test!(NotEqualsTo, false);

    generate_order_bytecode_test!(GreaterThan, true);
    generate_order_bytecode_test!(LesserThan, false);
    generate_order_bytecode_test!(GreaterEquals, true);
    generate_order_bytecode_test!(LesserEquals, false);
    
    #[allow(unused_imports)]
    mod jumps {
        use std::cell::RefCell;
        use azurite_common::Bytecode;
        use crate::{vm::VM, VMData};

        #[test]
        fn jump() {
            let mut vm = VM::new().unwrap();
            
            assert!(vm.run(&RefCell::new(vec![]), &[Bytecode::Jump as u8, 3, 255, 255, 255, Bytecode::Return as u8]).is_ok());
        }

        
        #[test]
        fn jump_if_false() {
            let mut vm = VM::new().unwrap();
            
            vm.stack.push(VMData::Bool(false)).unwrap();

            assert!(vm.run(&RefCell::new(vec![]), &[Bytecode::JumpIfFalse as u8, 3, 255, 255, 255, Bytecode::Return as u8]).is_ok());
            assert_eq!(vm.stack.top, 0);
        }
        

        #[test]
        fn jump_if_false_invalid_stack() {
            let mut vm = VM::new().unwrap();
            
            vm.stack.push(VMData::Integer(0)).unwrap();

            assert!(vm.run(&RefCell::new(vec![]), &[Bytecode::JumpIfFalse as u8, 3, 255, 255, 255, Bytecode::Return as u8]).is_err());
        }
        
        
        #[test]
        fn jump_back() {
            let mut vm = VM::new().unwrap();
            
            assert!(vm.run(&RefCell::new(vec![]), &[Bytecode::Jump as u8, 3, Bytecode::Return as u8, 255, 255, Bytecode::JumpBack as u8, 5]).is_ok());
        }

        
        #[test]
        fn jump_large() {
            let mut vm = VM::new().unwrap();

            let value = 3u16.to_le_bytes();
            assert!(vm.run(&RefCell::new(vec![]), &[Bytecode::JumpLarge as u8, value[0], value[1], 255, 255, 255, Bytecode::Return as u8]).is_ok());
        }

        
        #[test]
        fn jump_if_false_large() {
            let mut vm = VM::new().unwrap();
            
            vm.stack.push(VMData::Bool(false)).unwrap();

            let value = 3u16.to_le_bytes();
            assert!(vm.run(&RefCell::new(vec![]), &[Bytecode::JumpIfFalseLarge as u8, value[0], value[1], 255, 255, 255, Bytecode::Return as u8]).is_ok());
            assert_eq!(vm.stack.top, 0);
        }
        

        #[test]
        fn jump_if_false_large_invalid_stack() {
            let mut vm = VM::new().unwrap();
            
            vm.stack.push(VMData::Integer(0)).unwrap();

            let value = 3u16.to_le_bytes();
            assert!(vm.run(&RefCell::new(vec![]), &[Bytecode::JumpIfFalseLarge as u8, value[0], value[1], 255, 255, 255, Bytecode::Return as u8]).is_err());
        }
        
        
        #[test]
        fn jump_back_large() {
            let mut vm = VM::new().unwrap();

            let value = 6u16.to_le_bytes();
            assert!(vm.run(&RefCell::new(vec![]), &[Bytecode::Jump as u8, 3, Bytecode::Return as u8, 255, 255, Bytecode::JumpBackLarge as u8, value[0], value[1]]).is_ok());
        }
    }
}
