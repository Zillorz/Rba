use std::collections::HashMap;
use codegen::ir::UserFuncName;
use cranelift::prelude::*;
use cranelift_codegen::data_value::DataValue;
use cranelift_codegen::gimli::ReaderOffset;
use cranelift_codegen::ir::{DynamicStackSlotData, DynamicType};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{default_libcall_names, Linkage, Module};

const PTR_LEN: usize = 8;
const N_TYPE: Type = types::I64;

type Word = u64;
type Addr = Word;
type Label = &'static str;

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum Var {
    A,
    B,
    C,
    Addr(Addr)
}

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum Val {
    Var(Var),
    Const(Word)
}

#[derive(Copy, Clone, Debug)]
#[repr(u8)]
pub enum MpIns {
    Move(Val, Var),
    Swap(Var, Var),
    Add(Var, Val),
    Sub(Var, Val),
    Mul(Var, Val),
    Div(Var, Val),
    Mod(Var, Val),
    Label(Label),
    JZ(Val, Label),
    JNz(Val, Label),
    TakeInput,
    CopyInput,
    Nop,
    Output(Val)
}

use cranelift_codegen::ir::stackslot::StackSize;

fn printc(val: Word) {
    println!("{val}")
}

// uses cranelift to generate x86 asm, faster than interpreting this processors instructions
pub fn into_cr(ins: &[MpIns], stack_size: u32) -> extern "C" fn() {
    let mut flag_builder = settings::builder();
    flag_builder.set("use_colocated_libcalls", "false").unwrap();
    // FIXME set back to true once the x64 backend supports it.
    flag_builder.set("is_pic", "false").unwrap();
    flag_builder.set("opt_level", "speed").unwrap();
    let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
        panic!("host machine is not supported: {}", msg);
    });

    let flags = settings::Flags::new(flag_builder);
    let isa = isa_builder
        .finish(flags)
        .unwrap();

    let mut builder = JITBuilder::with_isa(isa, default_libcall_names());
    let print_addr = printc as *const u8;
    builder.symbol("printc", print_addr);

    let mut module = JITModule::new(builder);

    let mut ctx = module.make_context();
    let mut func_ctx = FunctionBuilderContext::new();

    let sig_main = module.make_signature();

    let func_main = module
        .declare_function("main", Linkage::Export, &sig_main)
        .unwrap();

    ctx.func.signature = sig_main;
    ctx.func.name = UserFuncName::user(0, func_main.as_u32());

    let mut sig = module.make_signature();
    sig.params.push(AbiParam::new(N_TYPE));

    let callee = module
        .declare_function("printc", Linkage::Import, &sig)
        .map_err(|e| e.to_string()).unwrap();

    {
        let local_callee = module
            .declare_func_in_func(callee, &mut ctx.func);

        let mut bcx = FunctionBuilder::new(&mut ctx.func, &mut func_ctx);
        let block = bcx.create_block();

        let stack_slot = bcx.create_sized_stack_slot(StackSlotData::new(StackSlotKind::ExplicitSlot, StackSize::from_u32(stack_size)));
        bcx.switch_to_block(block);
        let a = Variable::new(0);
        let b = Variable::new(1);
        let c = Variable::new(2);

        bcx.declare_var(a, N_TYPE);
        bcx.declare_var(b, N_TYPE);
        bcx.declare_var(c, N_TYPE);
        let mut lookup = HashMap::new();

        let mut get_value = |r: Val, bcx: &mut FunctionBuilder| {
            match r {
                Val::Var(v) => {
                    match v {
                        Var::A => { bcx.use_var(a) }
                        Var::B => { bcx.use_var(b) }
                        Var::C => { bcx.use_var(c) }
                        Var::Addr(addr) => { bcx.ins().stack_load(N_TYPE, stack_slot, addr as i32) }
                    }
                }
                Val::Const(c) => { bcx.ins().iconst(N_TYPE, c as i64) }
            }
        };

        let mut get_var = |v: Var, bcx: &mut FunctionBuilder| {
            match v {
                Var::A => { bcx.use_var(a) }
                Var::B => { bcx.use_var(b) }
                Var::C => { bcx.use_var(c) }
                Var::Addr(addr) => { bcx.ins().stack_load(N_TYPE, stack_slot, addr as i32) }
            }
        };

        let mut set_var = |v: Var, val: Value, bcx: &mut FunctionBuilder| {
            match v {
                Var::A => { bcx.def_var(a, val) }
                Var::B => { bcx.def_var(b, val) }
                Var::C => { bcx.def_var(c, val) }
                Var::Addr(addr) => { bcx.ins().stack_store(val, stack_slot, addr as i32); }
            }
        };

        for i in ins {
            match *i {
                MpIns::Move(val, var) => {
                    let val = get_value(val, &mut bcx);
                    set_var(var, val, &mut bcx);
                }
                MpIns::Swap(vr1, vr2) => {
                    let r1 = get_var(vr1, &mut bcx);
                    let r2 = get_var(vr2, &mut bcx);

                    set_var(vr2, r1, &mut bcx);
                    set_var(vr1, r2, &mut bcx);
                }
                MpIns::Add(var, val) => {
                    let v1 = get_var(var, &mut bcx);
                    let v2 = get_value(val, &mut bcx);

                    let v3 = bcx.ins().iadd(v1, v2);
                    set_var(var, v3, &mut bcx);
                }
                MpIns::Sub(var, val) => {
                    let v1 = get_var(var, &mut bcx);
                    let v2 = get_value(val, &mut bcx);

                    let v3 = bcx.ins().isub(v1, v2);
                    set_var(var, v3, &mut bcx);
                }
                MpIns::Mul(var, val) => {
                    let v1 = get_var(var, &mut bcx);
                    let v2 = get_value(val, &mut bcx);

                    let v3 = bcx.ins().imul(v1, v2);
                    set_var(var, v3, &mut bcx);
                }
                MpIns::Div(var, val) => {
                    let v1 = get_var(var, &mut bcx);
                    let v2 = get_value(val, &mut bcx);

                    let v3 = bcx.ins().udiv(v1, v2);
                    set_var(var, v3, &mut bcx);
                }
                MpIns::Mod(var, val) => {
                    let v1 = get_var(var, &mut bcx);
                    let v2 = get_value(val, &mut bcx);

                    let v3 = bcx.ins().urem(v1, v2);
                    set_var(var, v3, &mut bcx);
                }
                MpIns::Label(id) => {
                    let bl = bcx.create_block();
                    lookup.insert(id, bl);
                    bcx.ins().jump(bl, &[]);

                    bcx.insert_block_after(bl, bcx.current_block().unwrap());
                    bcx.switch_to_block(bl);
                }
                MpIns::JZ(val, addr) => {
                    let bl = lookup.get(&addr).unwrap().clone();
                    let eb = bcx.create_block();

                    let bool = get_value(val, &mut bcx);
                    bcx.ins().brif(bool, eb, &[], bl, &[]);
                    bcx.switch_to_block(eb);
                    // bcx.seal_block(eb);
                }
                MpIns::JNz(val, addr) => {
                    let bl = lookup.get(&addr).unwrap().clone();
                    let eb = bcx.create_block();

                    let bool = get_value(val, &mut bcx);
                    bcx.ins().brif(bool, bl, &[], eb, &[]);
                    bcx.switch_to_block(eb);
                    // bcx.seal_block(eb);
                }
                // Input not yet implemented
                MpIns::TakeInput => {}
                MpIns::CopyInput => {}
                MpIns::Nop => {}
                MpIns::Output(val) => {
                    let val= get_value(val, &mut bcx);
                    bcx.ins().call(local_callee, &[val]);
                }
            }
        }

        bcx.ins().return_(&[]);
        bcx.seal_all_blocks();
        bcx.finalize();
    }

    module.define_function(func_main, &mut ctx).unwrap();
    module.clear_context(&mut ctx);

    module.finalize_definitions().unwrap();

    let code_main = module.get_finalized_function(func_main);
    let ret = unsafe { std::mem::transmute::<_, extern "C" fn()>(code_main) };

    ret
}

pub fn execute(ram: &mut [u8], ins: &[MpIns]) {
    let mut regs = [0, 0, 0];

    let mut lookup = HashMap::new();

    for (idx, i) in ins.iter().enumerate() {
        match i {
            MpIns::Label(addr) => { lookup.insert(*addr, idx + 1); }
            _ => { }
        }
    }

    let mut idx = 0;
    loop {
        if idx >= ins.len() { break; }
        let ir = run_ins(ins[idx], ram, &mut regs);

        match ir {
            InsResult::Rewind(pos) => { idx = *lookup.get(&pos).expect("Invalid goto"); }
            _ => { idx += 1; }
        }
    }
}

#[derive(Copy, Clone, Eq, PartialEq, Hash)]
enum InsResult {
    Failure,
    Success,
    Rewind(Label)
}

fn run_ins(ins: MpIns, memory: &mut [u8], rgs: &mut [Word; 3]) -> InsResult {
    let get_var = |v: Var, registers: &[Word; 3], mem: &mut [u8]| {
        match v {
            Var::A => { registers[0] }
            Var::B => { registers[1] }
            Var::C => { registers[2] }
            Var::Addr(addr) => {
                let addr = addr as usize;
                Word::from_le_bytes(mem[addr..addr + PTR_LEN].try_into().unwrap())
            }
        }
    };

    let mut set_var = |v: Var, val: Word, registers: &mut [Word; 3], mem: &mut [u8]| {
        match v {
            Var::A => { registers[0] = val; }
            Var::B => { registers[1] = val; }
            Var::C => { registers[2] = val; }
            Var::Addr(addr) => {
                let addr = addr as usize;
                let m = &mut mem[addr..addr + PTR_LEN];
                m.copy_from_slice(&val.to_le_bytes());
            }
        }
    };

    let get_val = |v: Val, registers: &[Word; 3], mem: &mut [u8]| {
        match v {
            Val::Var(v) => { get_var(v, registers, mem) }
            Val::Const(w) => { w }
        }
    };

    match ins {
        MpIns::Move(val, var) => {
            let v = get_val(val, rgs,memory);
            set_var(var, v, rgs, memory);
        }
        MpIns::Swap(v1, v2) => {
            let r1 = get_var(v1, rgs, memory);
            let r2 = get_var(v2, rgs, memory);

            set_var(v2, r1, rgs, memory);
            set_var(v2, r2, rgs, memory);
        }
        MpIns::Add(var, val) => {
            let a = get_var(var, rgs, memory);
            let b = get_val(val, rgs, memory);
            set_var(var, a + b, rgs, memory);
        }
        MpIns::Sub(var, val) => {
            let a = get_var(var, rgs, memory);
            let b = get_val(val, rgs, memory);
            set_var(var, a - b, rgs, memory);
        }
        MpIns::Mul(var, val) => {
            let a = get_var(var, rgs, memory);
            let b = get_val(val, rgs, memory);
            set_var(var, a * b, rgs, memory);
        }
        MpIns::Div(var, val) => {
            let a = get_var(var, rgs, memory);
            let b = get_val(val, rgs, memory);
            set_var(var, a / b, rgs, memory);
        }
        MpIns::Mod(var, val) => {
            let a = get_var(var, rgs, memory);
            let b = get_val(val, rgs, memory);
            set_var(var, a % b, rgs, memory);
        }
        MpIns::JZ(val, addr) => {
            let val = get_val(val, rgs, memory);
            if val == 0 { return InsResult::Rewind(addr); }
        }
        MpIns::JNz(val, addr) => {
            let val = get_val(val, rgs, memory);
            if val != 0 { return InsResult::Rewind(addr); }
        }
        MpIns::Output(val) => {
            let v = get_val(val, rgs, memory);
            println!("{v}");
        }
        _ => {}
    }

    InsResult::Success
}