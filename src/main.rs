// building up layers of abstraction to go from nothing to
// high level language, microprocessor/asm to python level

use crate::asm::AsmIns as Ins;
use crate::modules::BorrowingModuleProvider;
use crate::parser::asm;
use std::env;
use std::fs::File;
use std::io::Read;

mod asm;
mod modules;
mod parser;

fn main() {
    let file = env::args().nth(1).unwrap();
    let mut file = File::open(file).unwrap();
    let mut s = String::new();
    file.read_to_string(&mut s).unwrap();

    // we leak the instructions too lmao
    let ins = asm()
        .parse(Box::leak(s.into_boxed_str()).as_bytes())
        .unwrap();

    // dbg!(&ins);

    jit(&ins);
    // interpret(&ins);
}

#[allow(dead_code)]
fn jit(ins: &[Ins]) {
    let v = asm::into_cr(ins, BorrowingModuleProvider);
    unsafe { v() };
}

#[allow(dead_code)]
fn interpret(ins: &[Ins]) {
    unsafe { asm::execute(ins, BorrowingModuleProvider) };
}
