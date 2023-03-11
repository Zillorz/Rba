// building up layers of abstraction to go from nothing to
// high level language, microprocessor/asm to python level

use std::fs::File;
use std::io::Read;
use std::time::Instant;
use crate::parser::asm;
use crate::processor::{MpIns as Ins, Var, Val};

mod processor;
mod parser;

fn main() {
    let mut file = File::open("inp.rbasm").unwrap();
    let mut s = String::new();
    file.read_to_string(&mut s).unwrap();

    let ins= asm().parse(Box::leak(s.into_boxed_str()).as_bytes()).unwrap();

    dbg!(&ins);

    jit(&ins, 4096);
    // interpret(ins, 4096);
}

fn jit(ins: &[Ins], stack_size: u32) {
    let c = Instant::now();
    let v = processor::into_cr(ins, stack_size);
    println!("Compilation Done, took {:?}", c.elapsed());

    let i = Instant::now();
    v();
    println!("Runtime took {:?}", i.elapsed());
}

fn interpret(ins: &[Ins], stack_size: u32) {
    let i = Instant::now();
    processor::execute(&mut vec![0; stack_size as usize], ins);
    println!("Interpreting took {:?}", i.elapsed());
}

