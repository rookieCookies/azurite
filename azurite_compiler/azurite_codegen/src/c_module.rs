use std::{fmt::Write, collections::{HashMap, BTreeMap}, sync::Arc};

use azurite_ast_to_ir::{Function, Variable, IR, Block, FunctionIndex, ExternFunction};
use common::{SymbolTable, DataType, GENERIC_START_SYMBOL, GENERIC_END_SYMBOL, SymbolIndex, Data};

use crate::{CodegenModule, CodeGen};

pub struct CModule<'a> {
    string: String,
    symbol_table: &'a mut SymbolTable,
    indent: usize,
    constants: &'a [Data],
    state: &'a CodeGen<Self>,
    
    function_map: HashMap<FunctionIndex, SymbolIndex>,
    extern_function_map: Vec<SymbolIndex>,
}


impl CodegenModule for CModule<'_> {
    fn codegen(
        state: crate::CodeGen<Self>,
        symbol_table: &mut common::SymbolTable, 
        externs: BTreeMap<SymbolIndex, Vec<ExternFunction>>, 
        functions: Vec<azurite_ast_to_ir::Function>,
        constants: &[Data],
    ) -> Vec<u8> {
        let mut codegen = CModule {
            string: String::new(),
            symbol_table,
            indent: 0,
            constants,
            function_map: HashMap::with_capacity(functions.len()),
            extern_function_map: Vec::with_capacity(externs.iter().map(|x| x.1.len()).sum()),
            state: &state,
        };

        let _ = writeln!(codegen.string, r#"#include <stdint.h>"#);
        let _ = writeln!(codegen.string, r#"#include <stdbool.h>"#);
        let _ = writeln!(codegen.string, r#"#include "string_obj.h""#);
        let _ = writeln!(codegen.string, r#"#include "runtime.h""#);
        let _ = writeln!(codegen.string, r#"typedef struct {{}} unit;"#);

        for e in externs {
            let _ = writeln!(codegen.string, r#"#include "{}.h""#, codegen.symbol_table.get(&e.0));

            for mut f in e.1 {
                if f.return_type == DataType::Empty {
                    let name = format!("extern_{}", codegen.identifier(&f.path));
                    let _ = writeln!(
                        codegen.string,
                        "unit extern_{}({}) {{ {}({}); return (unit) {{}}; }}",
                        codegen.identifier(&f.path),
                        f.args
                            .iter()
                            .enumerate()
                            .map(|x| format!("{} {}", codegen.to_string(x.1), Variable(x.0 as u32 + 1)))
                            .intersperse(", ".to_string())
                            .collect::<String>(),
                        
                        codegen.identifier(&f.path),
                        (0..f.args.len()).map(|x| Variable(x as u32 + 1).to_string()).intersperse(", ".to_string()).collect::<String>()
                    );

                    f.path = codegen.symbol_table.add(name);
                }
                if f.function_index.0 as usize > codegen.extern_function_map.len() {
                    codegen.extern_function_map.push(f.path);
                } else {
                    codegen.extern_function_map.insert(f.function_index.0 as usize, f.path)
                }
            }
        }


        for s in codegen.state.structures.iter() {
            if !s.1.is_used {
                continue
            }

            let _ = writeln!(
                codegen.string,
                "struct {};",
                codegen.identifier(s.0),
            );
        }


        for s in codegen.state.structures.iter() {
            if !s.1.is_used {
                continue
            }

            let _ = writeln!(
                codegen.string,
                "struct {} {{ size_t rc; {}}};",
                codegen.identifier(s.0),
                s.1.fields.iter().enumerate().map(|x| format!("{} _{}; ", codegen.to_string(x.1), x.0.to_string())).collect::<String>(),
            );
        }
            
    
        for f in functions.iter() {
            let _ = writeln!(
                codegen.string, 
                "{} {}({});", 
                codegen.to_string(&f.return_type), 
                codegen.identifier(&f.identifier), 
                f.arguments.iter().map(|x| codegen.to_string(x)).intersperse(", ".to_string()).collect::<String>()
            );


            codegen.function_map.insert(f.function_index, f.identifier);
        }


        for f in functions {
            codegen.codegen_function(f);
        }
        

        codegen.string.into_bytes()
    }
}


impl CModule<'_> {
    fn codegen_function(&mut self, mut f: Function) {
        let _ = writeln!(
            self.string, 
            "\n\n{} {}({})", 
            self.to_string(&f.return_type), 
            self.identifier(&f.identifier), 
            f.arguments
                .iter()
                .enumerate()
                .map(|x| format!(
                    "{} {}", 
                    self.to_string(x.1),
                    Variable(x.0 as u32 + 1)
                ))
                .intersperse(", ".to_string()).collect::<String>()
        );

        self.indent();
        // dbg!(&f);

        
        for (a, arg) in f.arguments.iter().enumerate() {
            if !arg.is_obj() {
                continue
            }

            self.rc_inc(&f, Variable(a as u32 + 1))
        }

        
        for r in f.register_lookup.iter().enumerate() {
            if r.0 != 0 && r.0 <= f.arguments.len() {
                continue;
            }
            
            let _ = writeln!(
                self.string,
                "{}{} {};",
                self.indentation(),
                self.to_string(r.1),
                Variable(r.0 as u32),
            );
        }

        
        for b in std::mem::take(&mut f.blocks) {
            self.basic_block(&f, b);
        }
        
        self.dedent();
    }


    fn basic_block(&mut self, f: &Function, b: Block) {
        let _ = writeln!(
            self.string,
            "{}:",
            b.block_index
        );


        for ir in b.instructions {
            self.ir(f, ir);
        }
        
        
        let _ = match b.ending {
            azurite_ast_to_ir::BlockTerminator::Goto(v) => writeln!(self.string, "{}goto {};", self.indentation(), v),
            

            azurite_ast_to_ir::BlockTerminator::SwitchBool { cond, op1, op2 } => writeln!(
                self.string, 
                "{}if ({}) {{ goto {}; }} else {{ goto {}; }}",
                self.indentation(),
                cond, op1, op2),

            
            azurite_ast_to_ir::BlockTerminator::Return => {
                for i in f.register_lookup.iter().enumerate().rev() {
                    self.rc_dec(f, Variable(i.0 as u32))
                }
        
                if f.return_type == DataType::Empty {
                    writeln!(self.string, "{}return (unit) {{}};", self.indentation())
                } else {
                    writeln!(self.string, "{}return {};", self.indentation(), Variable(0))
                }
            },
        };
    }
    

    fn ir(&mut self, f: &Function, ir: IR) {
        macro_rules! infix_operation {
            ($dst: expr, $left: expr, $right: expr, $infix: literal) => {
                writeln!(
                    self.string,
                    "{}{} = {} {} {};",
                    self.indentation(),
                    $dst,
                    $left,
                    $infix,
                    $right,
                )
            }
        }


        macro_rules! cast_operation {
            ($dst: expr, $val: expr, $cast_as: literal) => {
                writeln!(
                    self.string,
                    "{}{} = ({}){};",
                    self.indentation(),
                    $dst,
                    $cast_as,
                    $val,
                )
            }
        }

        
        let _ = match ir {
            IR::Copy { dst, src } => {
                self.rc_inc(f, dst);
                writeln!(
                    self.string,
                    "{}{dst} = {src};",
                    self.indentation(),
                )
            },

            
            IR::Swap { v1, v2 } => {
                writeln!(
                    self.string,
                    "\
                    {}{} _temp = {v1}; \
                    {}{v1} = {v2}; \
                    {}{v2} = _temp;
                    ",
                    self.indentation(),
                    self.to_string(&f.register_lookup[v1.0 as usize]),

                    self.indentation(),
                    self.indentation(),
                )
            },

            
            IR::Load { dst, data } => {
                writeln!(
                    self.string,
                    "{}{} = {};",
                    self.indentation(),
                    dst,
                    match self.constants[data as usize] {
                        Data::I8 (v) => v.to_string(),
                        Data::I16(v) => v.to_string(),
                        Data::I32(v) => v.to_string(),
                        Data::I64(v) => v.to_string(),
                        Data::U8 (v) => v.to_string(),
                        Data::U16(v) => v.to_string(),
                        Data::U32(v) => v.to_string(),
                        Data::U64(v) => v.to_string(),
                        Data::Float(v) => v.to_string(),
                        Data::String(v) => {
                            let string = self.symbol_table.get(&v);

                            // +1 is for the null byte
                            let len = string.len() + 1;

                            format!("new_string(\"{}\", {len})", string)
                        },
                        Data::Bool(v) => v.to_string(),
                        Data::Empty => "void".to_string(),
                    }
                )
            },

            
            IR::Unit { .. } => return,

            
            IR::Add      { dst, left, right } => infix_operation!(dst, left, right, "+"),
            IR::Subtract { dst, left, right } => infix_operation!(dst, left, right, "-"),
            IR::Multiply { dst, left, right } => infix_operation!(dst, left, right, "*"),
            IR::Divide   { dst, left, right } => infix_operation!(dst, left, right, "/"),
            IR::Modulo   { dst, left, right } => infix_operation!(dst, left, right, "%"),
            IR::Equals   { dst, left, right } => infix_operation!(dst, left, right, "=="),
            IR::NotEquals { dst, left, right }     => infix_operation!(dst, left, right, "!="),
            IR::GreaterThan { dst, left, right }   => infix_operation!(dst, left, right, ">"),
            IR::LesserThan { dst, left, right }    => infix_operation!(dst, left, right, "<"),
            IR::GreaterEquals { dst, left, right } => infix_operation!(dst, left, right, ">="),
            IR::LesserEquals { dst, left, right }  => infix_operation!(dst, left, right, "<="),

            
            IR::UnaryNot { dst, val } => write!(self.string, "{}{dst} = !{val};", self.indentation()),
            IR::UnaryNeg { dst, val } => write!(self.string, "{}{dst} = -{val};", self.indentation()),
            
            
            IR::Call { dst, id, args } => {
                writeln!(
                    self.string,
                    "{}{dst} = {}( {} );",
                    self.indentation(),
                    self.identifier(self.function_map.get(&id).unwrap()),
                    args.into_iter().map(|x| x.to_string()).intersperse(", ".to_string()).collect::<String>()
                )
            },

            
            IR::ExtCall { dst, id, args } => {
                writeln!(
                    self.string,
                    "{}{dst} = {}( {} );",
                    self.indentation(),
                    self.identifier(&self.extern_function_map[id.0 as usize]),
                    args.into_iter().map(|x| x.to_string()).intersperse(", ".to_string()).collect::<String>()
                )
            },

            
            IR::Struct { dst, fields, id } => {
                let indent = self.indentation();
                let name = self.to_string(&DataType::Struct(id, Arc::new([])));

                let _ = writeln!(self.string, "{}{dst} = alloc(sizeof({name}));", indent);
                let _ = writeln!(self.string, "{}{dst}->rc = 0;", indent);
                
                for (i, f) in fields.iter().enumerate() {
                    let _ = writeln!(self.string, "{}{dst}->_{i} = {f};", indent);
                }

                self.rc_inc(f, dst);

                return
            },

            
            IR::AccStruct { dst, val, index } => {
                let name = self.to_string(&f.register_lookup[val.0 as usize]);
                let _ = writeln!(
                    self.string,
                    "{}{dst} = (({name}){val})->_{index};",
                    self.indentation()
                );
                
                self.rc_inc(f, dst);
                return
            },


            IR::SetField { dst, data, index } => {
                let structure = &f.register_lookup[dst.0 as usize];
                let typ = match structure {
                    DataType::Struct(v, _) => &self.state.structures.get(v).unwrap().fields[index as usize],
                    _ => unreachable!()
                };

                self.rc_recursive("rc_dec", format!("{dst}._{index}"), typ);
                writeln!(
                    self.string,
                    "{}{dst}._{index} = {data};",
                    self.indentation(),
                )
            },

            
            IR::CastToI8 { dst, val }    => cast_operation!(dst, val, "uint8_t"),
            IR::CastToI16 { dst, val }   => cast_operation!(dst, val, "uint16_t"),
            IR::CastToI32 { dst, val }   => cast_operation!(dst, val, "uint32_t"),
            IR::CastToI64 { dst, val }   => cast_operation!(dst, val, "uint64_t"),
            IR::CastToU8 { dst, val }    => cast_operation!(dst, val, "uint8_t"),
            IR::CastToU16 { dst, val }   => cast_operation!(dst, val, "uint16_t"),
            IR::CastToU32 { dst, val }   => cast_operation!(dst, val, "uint32_t"),
            IR::CastToU64 { dst, val }   => cast_operation!(dst, val, "uint64_t"),
            IR::CastToFloat { dst, val } => cast_operation!(dst, val, "float"),

            IR::Noop => return,
        };
    }
}


impl CModule<'_> {
    fn to_string(&self, datatype: &DataType) -> String {
        match datatype {
            DataType::I8  => "int8_t".to_string(),
            DataType::I16 => "int16_t".to_string(),
            DataType::I32 => "int32_t".to_string(),
            DataType::I64 => "int64_t".to_string(),
            DataType::U8  => "uint8_t".to_string(),
            DataType::U16 => "uint16_t".to_string(),
            DataType::U32 => "uint32_t".to_string(),
            DataType::U64 => "uint64_t".to_string(),
            DataType::Float => "float".to_string(),
            DataType::String => "string*".to_string(),
            DataType::Bool => "bool".to_string(),
            DataType::Empty => "unit".to_string(),
            DataType::Any => panic!("uh oh"),
            DataType::Struct(_, _) => format!("struct {}*", datatype.to_string(self.symbol_table).replace("::", "_").replace(GENERIC_START_SYMBOL, "🚀").replace(GENERIC_END_SYMBOL, "🥓")),
        }
    }


    fn identifier(&self, identifier: &SymbolIndex) -> String {
        self.symbol_table.get(identifier).replace("::", "_").replace(GENERIC_START_SYMBOL, "🚀").replace(GENERIC_END_SYMBOL, "🥓")
    }


    fn indent(&mut self) {
        let _ = writeln!(self.string, "{}{{", self.indentation());
        self.indent += 1;
    }

    
    fn dedent(&mut self) {
        self.indent -= 1;
        let _ = writeln!(self.string, "{}}}", self.indentation());
    }


    fn indentation(&self) -> String {
        "\t".repeat(self.indent)
    }


    fn rc_inc(&mut self, f: &Function, reg: Variable) {
        self.rc_recursive("rc_inc", reg.to_string(), f.register_lookup.get(reg.0 as usize).unwrap())
    }

    
    fn rc_dec(&mut self, f: &Function, reg: Variable) {
        let typ = f.register_lookup.get(reg.0 as usize).unwrap();
        if typ.is_obj() {
            self.rc_recursive("rc_dec", reg.to_string(), typ);
        }
    }


    fn rc_recursive(&mut self, op: &str, var: String, typ: &DataType) {
        if let DataType::Struct(v, _) = typ {
            let fields = &self.state.structures.get(v).unwrap().fields;
            if !fields.iter().any(|x| x.is_obj()) {
                 let _ = writeln!(
                    self.string,
                    "{}{op}((size_t*) {var});",
                    self.indentation(),
                );
                return
            }

            self.indent();
            for (index, ftyp) in fields.iter().enumerate() {
                if !ftyp.is_obj() {
                    continue
                }

                let _ = writeln!(
                    self.string,
                    "{}{} {var}{} = {var}->_{index};",
                    self.indentation(),
                    self.to_string(ftyp),
                    Variable(index as u32),
                );

                self.rc_recursive(op, format!("{var}{}", Variable(index as u32)), ftyp);
            }

            self.dedent();
        }
        
         let _ = writeln!(
            self.string,
            "{}{op}((size_t*) {var});",
            self.indentation(),
        );

    }
}
