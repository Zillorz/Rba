// building up layers of abstraction to go from nothing to
// high level language, microprocessor/asm to python level

use std::env;
use std::fs::File;
use std::io::Read;
use std::time::Instant;
use crate::parser::{asm, fcall};
use crate::asm::{AsmIns as Ins, Var, Val};
use crate::modules::{BorrowingModuleProvider, DefaultModuleProvider};

mod asm;
mod parser;
mod modules;

fn main() {
    let file = env::args().nth(1).unwrap();
    let mut file = File::open(file).unwrap();
    let mut s = String::new();
    file.read_to_string(&mut s).unwrap();
    let ins = asm().parse(Box::leak(s.into_boxed_str()).as_bytes()).unwrap();

    dbg!(&ins);

    jit(&ins);
    // interpret(&ins);
}

fn jit(ins: &[Ins]) {
    let v = asm::into_cr(ins, BorrowingModuleProvider);
    unsafe { v() };
}

fn interpret(ins: &[Ins]) {
    unsafe { asm::execute(ins) };
}

