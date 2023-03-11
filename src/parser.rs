use pom::parser::*;
use pom::Parser;

use std::collections::HashMap;
use std::str::{self, FromStr};
use crate::processor::{MpIns, Val, Var};

fn space() -> Parser<u8, ()> {
    one_of(b" \t\r\n").repeat(0..).discard()
}

fn number() -> Parser<u8, u64> {
    let integer = one_of(b"123456789") - one_of(b"0123456789").repeat(0..) | sym(b'0');
    integer.collect().convert(str::from_utf8).convert(|s| u64::from_str(&s))
}


fn val() -> Parser<u8, Val> {
    var().map(Val::Var) | number().map(Val::Const)
}

fn var() -> Parser<u8, Var> {
        sym(b'A').map(|_| Var::A)
    |   sym(b'B').map(|_| Var::B)
    |   sym(b'C').map(|_| Var::C)
    | sym(b'&') * number().map(Var::Addr)
}

fn mov() -> Parser<u8, (Val, Var)> {
    space() * val() + space() * var()
}

fn op() -> Parser<u8, (Var, Val)> {
    space() * var() + space() * val()
}

fn swap() -> Parser<u8, (Var, Var)> {
    space() * var() + space() * var()
}

fn make_static<'a>(s: &'a str) -> &'static str {
    Box::leak(String::from(s).into_boxed_str())
}

fn label() -> Parser<u8, &'static str> {
    space() * none_of(b" ;").repeat(0..).collect().convert(str::from_utf8).map(make_static)
}

fn jmp() -> Parser<u8, (Val, &'static str)> {
    space() * val() + space() * label()
}

fn ins() -> Parser<u8, MpIns> {
    space() * (
            seq(b"MOV") * mov().map(|(a, b)| MpIns::Move(a, b))
        |   seq(b"SWAP") * swap().map(|(a, b)| MpIns::Swap(a, b))
        |   seq(b"ADD") * op().map(|(a, b)| MpIns::Add(a, b))
        |   seq(b"SUB") * op().map(|(a, b)| MpIns::Sub(a, b))
        |   seq(b"MUL") * op().map(|(a, b)| MpIns::Mul(a, b))
        |   seq(b"DIV") * op().map(|(a, b)| MpIns::Div(a, b))
        |   seq(b"MOD") * op().map(|(a, b)| MpIns::Mod(a, b))
        |   seq(b"LABEL:") * space() * label().map(MpIns::Label)
        |   seq(b"JZ") * jmp().map(|(a, b)| MpIns::JZ(a, b))
        |   seq(b"JNZ") * jmp().map(|(a, b)| MpIns::JNz(a, b))
        |   seq(b"OUT") * space() * val().map(MpIns::Output)
        |   seq(b"NOP") * space().map(|_| MpIns::Nop)
        ) - space()
}

pub fn asm() -> Parser<u8, Vec<MpIns>> {
    list(ins(), sym(b';'))
}