use std::collections::HashMap;
use std::ops::Deref;
use codegen::ir::UserFuncName;
use cranelift::prelude::*;
use cranelift_codegen::Context;
use cranelift_codegen::data_value::DataValue;
use cranelift_codegen::gimli::ReaderOffset;
use cranelift_codegen::ir::{DynamicStackSlotData, DynamicType, StackSlot};
use cranelift_codegen::settings::{self, Configurable};
use cranelift_jit::{JITBuilder, JITModule};
use cranelift_module::{default_libcall_names, FuncId, Linkage, Module};
use crate::modules::{DefaultModuleProvider, Module as AModule, ModuleProvider};

const PTR_LEN: usize = 8;
const N_TYPE: Type = types::I64;

pub(crate) type Word = u64;
pub(crate) type Addr = Word;
type Label = String;

#[derive(Clone, Debug)]
#[repr(u8)]
pub enum Var {
    Named(Label),
    Addr(Box<Val>)
}

#[derive(Clone, Debug)]
#[repr(u8)]
pub enum Val {
    Var(Var),
    Const(Const)
}

#[derive(Clone, Debug)]
#[repr(u8)]
pub enum Const {
    Word(Word),
    Str(*const u8)
}

#[derive(Clone, Debug)]
#[repr(u8)]
pub enum AsmIns {
    Include(Label),
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
    Output(Val),
    Call(Label, Vec<Val>, Option<Var>),
    Function(Label, Vec<AsmIns>)
}

use cranelift_codegen::isa::{Builder, LookupError, OwnedTargetIsa};
use libc::{c_char, c_int, size_t};
use pom::Parser;

fn printc(val: Word) { println!("{val}") }
fn print_ascii(val: Word) { print!("{}", char::from_u32(val as u32).unwrap()) }
fn top_8(val: Word) -> Word { val >> 56 }

// uses cranelift to generate x86 asm, faster than interpreting this processors instructions
pub fn into_cr<M: ModuleProvider>(ins: &[AsmIns], provider: M) -> unsafe extern "C" fn() {
    let mut flag_builder = settings::builder();
    flag_builder.set("use_colocated_libcalls", "false").unwrap();
    // FIXME set back to true once the x64 backend supports it.
    flag_builder.set("is_pic", "true").unwrap();
    flag_builder.set("opt_level", "speed").unwrap();
    let isa_builder = cranelift_native::builder().unwrap_or_else(|msg| {
        panic!("host machine is not supported: {}", msg);
    });

    let flags = settings::Flags::new(flag_builder);
    let isa = isa_builder
        .finish(flags)
        .unwrap();

    let mut builder = JITBuilder::with_isa(isa.clone(), default_libcall_names());
    let printc_addr = printc as *const u8;
    builder.symbol("printc", printc_addr);

    for i in ins {
        match i {
            AsmIns::Include(lib) => {
                provider.add_functions(&mut builder, lib);
            }
            _ => { }
        }
    }

    let mut module = JITModule::new(builder);

    let mut ctx = module.make_context();
    let mut func_ctx = FunctionBuilderContext::new();

    // every function has its own Memory space
    // Determine arg strategy soon, maybe arg1 arg2 arg3?
    fn make_function(name: &str, signature: Signature,
                     module: &mut JITModule, ctx: &mut Context, func_ctx: &mut FunctionBuilderContext, ins: &[AsmIns]) -> FuncId {
        let func_s = module
            .declare_function(name, Linkage::Export, &signature)
            .unwrap();

        let mut sig = module.make_signature();
        sig.params.push(AbiParam::new(N_TYPE));

        let callee = module
            .declare_function("printc", Linkage::Import, &sig)
            .map_err(|e| e.to_string()).unwrap();

        ctx.func.signature = signature;
        ctx.func.name = UserFuncName::user(0, func_s.as_u32());

        let local_callee = module
            .declare_func_in_func(callee, &mut ctx.func);

        let mut function_lookup = HashMap::new();

        for i in ins {
            match i {
                AsmIns::Call(label, params, out) => {
                    if function_lookup.contains_key(label) { continue; }

                    let mut sig = module.make_signature();
                    for _ in 0..params.len() {
                        sig.params.push(AbiParam::new(N_TYPE));
                    }
                    if out.is_some() { sig.returns.push(AbiParam::new(N_TYPE)); }

                    let callee = module
                        .declare_function(label, Linkage::Import, &sig)
                        .map_err(|e| e.to_string()).unwrap();

                    let func_ref = module.declare_func_in_func(callee, &mut ctx.func);

                    function_lookup.insert(label, func_ref);
                }
                _ => { }
            }
        }

        let mut bcx = FunctionBuilder::new(&mut ctx.func, func_ctx);
        let block = bcx.create_block();
        bcx.switch_to_block(block);

        let mut vidx = 0;
        let mut variable_lookup = HashMap::new();
        let mut block_lookup = HashMap::new();

        struct Env {
            vl: HashMap<Label, Variable>,
            vi: usize
        }

        let mut env = Env {
            vl: variable_lookup,
            vi: vidx
        };

        fn get_val1(v: Val, bcx: &mut FunctionBuilder, env: &mut Env) -> Value {
            match v {
                Val::Var(v) => { get_var1(v, bcx, env) }
                Val::Const(Const::Word(word)) => {
                    bcx.ins().iconst(N_TYPE, word as i64)
                }
                Val::Const(Const::Str(string)) => {
                    bcx.ins().iconst(N_TYPE, string as i64)
                }
            }
        }

        fn get_var1(v: Var, bcx: &mut FunctionBuilder, env: &mut Env) -> Value {
            match v {
                Var::Named(label) => {
                    if let Some(v) = env.vl.get(&label) {
                        bcx.use_var(v.clone())
                    } else {
                        let v = Variable::new(env.vi);
                        env.vi += 1;

                        env.vl.insert(label, v.clone());
                        bcx.declare_var(v, N_TYPE);
                        bcx.use_var(v)
                    }
                }
                Var::Addr(bval) => {
                    let addr = get_val1(*bval, bcx, env);
                    bcx.ins().load(N_TYPE, MemFlags::new(), addr, 0)
                }
            }
        }

        fn set_var1(var: Var, to: Value,  bcx: &mut FunctionBuilder, env: &mut Env) {
            match var {
                Var::Named(label) => {
                    if let Some(v) = env.vl.get(&label) {
                        bcx.def_var(v.clone(), to)
                    } else {
                        let v = Variable::new(env.vi);
                        env.vi += 1;

                        env.vl.insert(label, v.clone());
                        bcx.declare_var(v, N_TYPE);
                        bcx.def_var(v, to)
                    }
                }
                Var::Addr(bval) => {
                    let addr = get_val1(*bval, bcx, env);
                    bcx.ins().store(MemFlags::new(), to, addr, 0);
                }
            }
        }

        let mut get_value = |r: &Val, bcx: &mut FunctionBuilder, env: &mut Env| { get_val1(r.clone(), bcx, env) };
        let mut get_var = |v: &Var, bcx: &mut FunctionBuilder, env: &mut Env| { get_var1(v.clone(), bcx, env) };
        let mut set_var = |v: &Var, val: Value, bcx: &mut FunctionBuilder, env: &mut Env| { set_var1(v.clone(), val, bcx, env) };

        for i in ins {
            match i {
                AsmIns::Label(id) => {
                    let bl = bcx.create_block();
                    block_lookup.insert(id, bl);
                }
                _ => { }
            }
        }

        for i in ins {
            match i {
                AsmIns::Move(val, var) => {
                    let val = get_value(val, &mut bcx, &mut env);
                    set_var(var, val, &mut bcx, &mut env);
                }
                AsmIns::Swap(vr1, vr2) => {
                    let r1 = get_var(vr1, &mut bcx, &mut env);
                    let r2 = get_var(vr2, &mut bcx, &mut env);

                    set_var(vr2, r1, &mut bcx, &mut env);
                    set_var(vr1, r2, &mut bcx, &mut env);
                }
                AsmIns::Add(var, val) => {
                    let v1 = get_var(var, &mut bcx, &mut env);
                    let v2 = get_value(val, &mut bcx, &mut env);

                    let v3 = bcx.ins().iadd(v1, v2);
                    set_var(var, v3, &mut bcx, &mut env);
                }
                AsmIns::Sub(var, val) => {
                    let v1 = get_var(var, &mut bcx, &mut env);
                    let v2 = get_value(val, &mut bcx, &mut env);

                    let v3 = bcx.ins().isub(v1, v2);
                    set_var(var, v3, &mut bcx, &mut env);
                }
                AsmIns::Mul(var, val) => {
                    let v1 = get_var(var, &mut bcx, &mut env);
                    let v2 = get_value(val, &mut bcx, &mut env);

                    let v3 = bcx.ins().imul(v1, v2);
                    set_var(var, v3, &mut bcx, &mut env);
                }
                AsmIns::Div(var, val) => {
                    let v1 = get_var(var, &mut bcx, &mut env);
                    let v2 = get_value(val, &mut bcx, &mut env);

                    let v3 = bcx.ins().udiv(v1, v2);
                    set_var(var, v3, &mut bcx, &mut env);
                }
                AsmIns::Mod(var, val) => {
                    let v1 = get_var(var, &mut bcx, &mut env);
                    let v2 = get_value(val, &mut bcx, &mut env);

                    let v3 = bcx.ins().urem(v1, v2);
                    set_var(var, v3, &mut bcx, &mut env);
                }
                AsmIns::JZ(val, addr) => {
                    let bl = block_lookup.get(addr).unwrap().clone();
                    let eb = bcx.create_block();

                    let bool = get_value(val, &mut bcx, &mut env);
                    bcx.ins().brif(bool, eb, &[], bl, &[]);
                    bcx.switch_to_block(eb);
                    // bcx.seal_block(eb);
                }
                AsmIns::JNz(val, addr) => {
                    let bl = block_lookup.get(&addr).unwrap().clone();
                    let eb = bcx.create_block();

                    let bool = get_value(val, &mut bcx, &mut env);
                    bcx.ins().brif(bool, bl, &[], eb, &[]);
                    bcx.switch_to_block(eb);
                    // bcx.seal_block(eb);
                }
                AsmIns::Label(id) => {
                    let bl = block_lookup.get(&id).unwrap().clone();
                    bcx.ins().jump(bl, &[]);

                    bcx.insert_block_after(bl, bcx.current_block().unwrap());
                    bcx.switch_to_block(bl);
                }
                AsmIns::Output(val) => {
                    let val= get_value(val, &mut bcx, &mut env);
                    bcx.ins().call(local_callee, &[val]);
                }
                AsmIns::Call(label, params, ret) => {
                    let args: Vec<Value> = params.iter().map(|arg| get_value(arg, &mut bcx, &mut env)).collect();

                    let inst = bcx.ins().call(function_lookup.get(label).unwrap().clone(), &args);

                    if let Some(ret) = ret {
                        let out = bcx.inst_results(inst)[0];

                        set_var(ret, out, &mut bcx, &mut env);
                    }
                }
                _ => { }
            }
        }

        bcx.ins().return_(&[]);
        bcx.seal_all_blocks();
        bcx.finalize();

        func_s
    }

    let sig_main = module.make_signature();
    let func_main = make_function("main", sig_main, &mut module, &mut ctx, &mut func_ctx, ins);

    module.define_function(func_main, &mut ctx).unwrap();
    module.clear_context(&mut ctx);

    module.finalize_definitions().unwrap();


    let code_main = module.get_finalized_function(func_main);
    let ret = unsafe { std::mem::transmute::<_, unsafe extern "C" fn()>(code_main) };

    ret
}

pub unsafe fn execute(ins: &[AsmIns]) {
    let mut regs = HashMap::new();

    let mut lookup = HashMap::new();

    for (idx, i) in ins.iter().enumerate() {
        match i {
            AsmIns::Label(addr) => { lookup.insert(addr, idx + 1); }
            _ => { }
        }
    }

    let mut idx = 0;
    loop {
        if idx >= ins.len() { break; }
        let ir = run_ins(&ins[idx], &mut regs);

        match ir {
            InsResult::Rewind(pos) => { idx = *lookup.get(&pos).expect("Invalid jump"); }
            _ => { idx += 1; }
        }
    }
}

#[derive(Clone, Eq, PartialEq, Hash)]
enum InsResult {
    Failure,
    Success,
    Rewind(Label)
}

unsafe fn run_ins(ins: &AsmIns, rgs: &mut HashMap<String, Word>) -> InsResult {
    unsafe fn get_val1(v: Val, rgs: &mut HashMap<String, Word>) -> Word {
        match v {
            Val::Var(v) => { get_var1(v, rgs) }
            Val::Const(Const::Word(w)) => { w }
            Val::Const(Const::Str(ptr)) => { ptr as Word }
        }
    }

    unsafe fn get_var1(v: Var, rgs: &mut HashMap<String, Word>) -> Word {
        match v {
            Var::Named(lbl) => {
                if let Some(w) = rgs.get(&lbl) {
                    *w
                } else {
                    0
                }
            },
            Var::Addr(bval) => {
                let addr = get_val1(*bval, rgs) as usize;

                // wildly unsafe, but language specification demands it
                let u64 = unsafe { *std::mem::transmute::<usize, *const u64>(addr) };
                u64
            }
        }
    }

    unsafe fn set_var1(v: Var, to: Word, rgs: &mut HashMap<String, Word>) {
        match v {
            Var::Named(lbl) => {
                if let Some(w) = rgs.get_mut(&lbl) {
                    *w = to;
                } else {
                    rgs.insert(lbl, to);
                }
            }
            Var::Addr(bval) => {
                let addr = get_val1(*bval, rgs);
                let addr = addr as usize;
                
                // wildly unsafe, but language specification demands it
                let u64 = unsafe { std::mem::transmute::<usize, *mut u64>(addr) };
                
                unsafe {
                    *u64 = to;
                }
            }
        }
    }

    let get_val = |v: &Val, rgs: &mut HashMap<String, Word>| {
        get_val1(v.clone(), rgs)
    };

    let get_var = |v: &Var, rgs: &mut HashMap<String, Word>| {
        get_var1(v.clone(), rgs)
    };

    let mut set_var = |v: &Var, val: Word, rgs: &mut HashMap<String, Word>| {
        set_var1(v.clone(), val, rgs);
    };

    match ins {
        AsmIns::Move(val, var) => {
            let v = get_val(val, rgs);
            set_var(var, v, rgs);
        }
        AsmIns::Swap(v1, v2) => {
            let r1 = get_var(v1, rgs);
            let r2 = get_var(v2, rgs);

            set_var(v2, r1, rgs);
            set_var(v1, r2, rgs);
        }
        AsmIns::Add(var, val) => {
            let a = get_var(var, rgs);
            let b = get_val(val, rgs);
            set_var(var, a + b, rgs);
        }
        AsmIns::Sub(var, val) => {
            let a = get_var(var, rgs);
            let b = get_val(val, rgs);
            set_var(var, a - b, rgs);
        }
        AsmIns::Mul(var, val) => {
            let a = get_var(var, rgs);
            let b = get_val(val, rgs);
            set_var(var, a * b, rgs);
        }
        AsmIns::Div(var, val) => {
            let a = get_var(var, rgs);
            let b = get_val(val, rgs);
            set_var(var, a / b, rgs);
        }
        AsmIns::Mod(var, val) => {
            let a = get_var(var, rgs);
            let b = get_val(val, rgs);
            set_var(var, a % b, rgs);
        }
        AsmIns::JZ(val, addr) => {
            let val = get_val(val, rgs);
            if val == 0 { return InsResult::Rewind(addr.clone()); }
        }
        AsmIns::JNz(val, addr) => {
            let val = get_val(val, rgs);
            if val != 0 { return InsResult::Rewind(addr.clone()); }
        }
        AsmIns::Output(val) => {
            let v = get_val(val, rgs);
            println!("{v}");
        }
        AsmIns::Call(lbl, params, out) => {
            // gotta find a better way to do this
            // implement modules (already made in jit) ^ the better way to do this
            match lbl.as_str() {
                "malloc" => {
                    let out = out.clone().unwrap();
                    let ret = libc::malloc(get_val(&params[0], rgs) as usize);
                    set_var(&out, ret as usize as Word, rgs);
                },
                "printa" => {
                    print_ascii(get_val(&params[0], rgs));
                },
                "printc" => {
                    printc(get_val(&params[0], rgs));
                }
                _ => { }
            }
        }
        _ => {}
    }

    InsResult::Success
}