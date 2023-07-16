# 1.0 The structure of the azurite VM


## The `azurite` file format
The `.azurite` file format is a special format created by the compiler containing various information such as the constants and the bytecode


## Data Types
azurite as a language is strictly-typed at compile time and thus the VM expects all the type-checking to be done prior to runtime.  
Each type in azurite also has a *type id*


### References
A reference to an object is considered a *reference* value. More than one reference to an object can exist and references can be thought of pointers to their respective objects


### Primitives
Primitives by default have no specified default value and thus are assumed to not have a default value. The compiler must ensure that the primitive value is assigned in one way or another

Primitives include the following
- `unit`: a value which contains no information and is purely placeholder
- `i8`: a 8 bit signed integer
- `i16`: a 16 bit signed integer
- `i32`: a 32 bit signed integer
- `i64`: a 64 bit signed integer
- `u8 `: a 8 bit unsigned integer
- `u16`: a 16 bit unsigned integer
- `u32`: a 32 bit unsigned integer
- `u64`: a 64 bit unsigned integer
- `float`: a 64 but floating-point number 
- `bool`: a value which can either be *true* or *false*  
- `ref`: a value containing the *reference* type to an object


### Type ID
Type IDs, except for a few reserved for primitives, do not have a defined layout and should not be relied on between compilations  
Each new user-defined type in azurite **must** have a unique type ID and be consistent throughout the lifetime of the running program. It is undefined behaviour if this requirement can not be upheld by the compiler.  
The reserved type IDs are the following
- `0` for `unit`
- `1` for `i8`
- `2` for `i16`
- `3` for `i32`
- `4` for `i64`
- `5` for `u8`
- `6` for `u16`
- `7` for `u32`
- `8` for `u64`
- `9` for `f64`
- `10` for `bool`
- '11' for 'str' objects
- `12` to `256` (inclusive) reserved for future

Note: Some VMs might treat the "reserved" area as objects and thus it is up to the compiler to not allow any object to have an id below 256




## Runtime Memory


### The Stack
The azurite stack is a thread-local place where all the locals are located.  
Since the stack is never directly accessed except for pushing and popping of function frames it is not required to be contiguous and may be fragmented.  
The stack may be fixed-sized or dynamically-growing but it must have the following minimum values.  
Note: As there is no set-size for primitives the values provided below are provided in the unit of *primitives* and the memory required will scale with the size of the *primitives* in the implementation.

The stack of the *main thread* must be able to contain at least `65536` primitives  
The stack of the *any other side thread* must be able to contain at least `32768` primitives  
  
All of the memory for the stack is not required to be allocated at once and may grow as needed as long as it doesn't panic from stack overflow before hitting the minimums provided.  

If the stack reaches its limit or it is unable to grow for whatever reason it must throw a `stack overflow` error  
If there isn't enough memory to create the stack in the first place it must throw a `out of memory` error


### The `program counter`
Each thread of execution will have it's own `program counter` keeping track of which part of the bytecode it is at. The `program counter` must be big enough to contain a pointer on the native platform


### The Heap
The heap is created at start-up and is **not** thread local  

Objects in azurite live in the heap and have the primitive type *ref* reference them in the stack  

An object in the heap is never explicitly freed unless the `EXPLICIT_FREE` extension is provided by the VM implementation and thus musn't be relied on for correctness but may be used for optimisations.

The heap is garbage collected but the specification does not specify any garbage collection strategy and thus the implementation is free use any strategy.

The heap may grow in size as necessary 

If the heap runs out of memory and fails to resize it must throw a `out of memory` error


### Constant pool
The constant pool is a global read-only space that provides compile-time-known values such as literal numbers and some references  

The constant pool is created before any code is ran and is exactly the size provided by the `metadata` section of the `.azurite` file  

The constant pool must be in the order that is given in the `.azurite` file and musn't change the order

In the case of the constant pool not being able to be loaded because of memory it must throw a `out of memory` error


### Extern Function Array
The extern function array is a global array of native function pointers  

The array must be created before any code is ran and is exactly the size provided by the `metadata` section of the `.azurite` file  

The array will be populated by bytecode at runtime

In the case of the extern function array not being able to be loaded because of memory it must throw a `out of memory` error


### Stack Offset counter
The stack offset counter defines the "bottom" of the stack and any register accessed must be added to the stack offset before accessing the stack with the resulting value




## Call Stack
The call stack is a stack of Call Frames that gets pushed and popped with function calls and returns  

When the callstack is empty and encountered a pop it should stop code execution and begin terminating the thread  

### Call Frame
The call frame is a snapshot of the current frames state. It contains stuff like 
the stack offset and the current program counter  

### Current Frame
The current frame is a call frame that is currently being used/executed




# 2.0 The Bytecode
This section contains the specification for all of the bytecode instructions

## Return
8 bit code: 0
arguments: none

In the case of the call stack being empty, stop code execution and begin terminating the thread
In the case that there is a value in the call stack do the following
Must set the current 0th register to the current frame's return register and change the current
frame to the top value of the call stack, popping it by one


## Copy
8 bit code: 1
arguments: 
  - dst: `u8`
  - frm: `u8`

Copy the value of the register `frm` to `dst`


## Swap
8 bit code: 2
arguments:
  - v1: `u8`
  - v2: `u8`

Swaps the values located at the registers `v1` and `v2`


## Call
8 bit code: 3
arguments:
  - goto: `u32`
  - dst: `u8`
  - argc: `u8`
  - arg: [u8; argc]

