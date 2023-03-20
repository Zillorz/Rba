use pom::parser::*;
use pom::Parser;

use std::collections::HashMap;
use std::str::{self, FromStr};
use crate::asm::{AsmIns, Const, Val, Var};

fn space() -> Parser<u8, ()> {
    one_of(b" \t\r\n").repeat(0..).discard()
}

fn number() -> Parser<u8, u64> {
    let integer = one_of(b"123456789") - one_of(b"_0123456789").repeat(0..) | sym(b'0');
    integer.collect().convert(str::from_utf8).convert(|s| u64::from_str(&s.replace("_", "")))
}

fn val() -> Parser<u8, Val> {
    var().map(Val::Var) | number().map(|w| Val::Const(Const::Word(w))) | string().map(|x| {
        let ret = Val::Const(Const::Str(x.as_ptr()));
        std::mem::forget(x);
        ret
    })
}

fn string() -> Parser<u8, String> {
    let special_char = sym(b'\\') | sym(b'/') | sym(b'"')
        | sym(b'b').map(|_|b'\x08') | sym(b'f').map(|_|b'\x0C')
        | sym(b'n').map(|_|b'\n') | sym(b'r').map(|_|b'\r') | sym(b't').map(|_|b'\t');
    let escape_sequence = sym(b'\\') * special_char;
    let string = sym(b'"') * (none_of(b"\\\"") | escape_sequence).repeat(0..) - sym(b'"');
    string.convert(String::from_utf8)
}

fn var() -> Parser<u8, Var> {
    space() * none_of(b" \";&0123456789,").repeat(1..).collect().convert(str::from_utf8).map(String::from).map(Var::Named)
    | sym(b'&') * call(val).map(|x| Var::Addr(Box::new(x)))
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

fn label() -> Parser<u8, String> {
    space() * none_of(b" ;\"").repeat(0..).collect().convert(str::from_utf8).map(String::from)
}

fn jmp() -> Parser<u8, (Val, String)> {
    space() * val() + space() * label()
}

pub fn fcall() -> Parser<u8, (String, Vec<Val>, Option<Var>)> {
    (label() + space() * list(val(), sym(b',') * space()) - space() + var().opt()).map(|((a, b), c)| (a, b, c))
}

pub fn rcall() -> Parser<u8, (String, Option<Var>)> {
    label() + space() * space() * var().opt()
}


fn ins() -> Parser<u8, AsmIns> {
    space() * (
            (seq(b"INCLUDE") | seq(b"INC")) * space() * label().map(AsmIns::Include)
        |   seq(b"MOV") * mov().map(|(a, b)| AsmIns::Move(a, b))
        |   seq(b"SWAP") * swap().map(|(a, b)| AsmIns::Swap(a, b))
        |   seq(b"ADD") * op().map(|(a, b)| AsmIns::Add(a, b))
        |   seq(b"SUB") * op().map(|(a, b)| AsmIns::Sub(a, b))
        |   seq(b"MUL") * op().map(|(a, b)| AsmIns::Mul(a, b))
        |   seq(b"DIV") * op().map(|(a, b)| AsmIns::Div(a, b))
        |   seq(b"MOD") * op().map(|(a, b)| AsmIns::Mod(a, b))
        |   seq(b"LABEL:") * space() * label().map(AsmIns::Label)
        |   seq(b"JZ") * jmp().map(|(a, b)| AsmIns::JZ(a, b))
        |   seq(b"JNZ") * jmp().map(|(a, b)| AsmIns::JNz(a, b))
        |   seq(b"CALL") * fcall().map(|(a, b, c)| AsmIns::Call(a, b, c))
        |   seq(b"RCALL") * rcall().map(|(a, b)| AsmIns::Call(a, Vec::new(), b))
        |   seq(b"OUT") * space() * val().map(AsmIns::Output)
        |   seq(b"NOP").map(|_| AsmIns::Nop)
    ) - space()
}

pub fn asm() -> Parser<u8, Vec<AsmIns>> {
    list(ins(), sym(b';'))
}