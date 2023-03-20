use std::collections::HashMap;
use std::fs::File;
use std::io::{Read, Write};
use std::ops::AddAssign;
use std::os::windows::io::FromRawHandle;
use std::process::Stdio;
use std::ptr;
use cranelift_jit::JITBuilder;
use rba_derive::module;
use crate::asm::{Addr, Word};

pub trait Module<K: Into<String>, T: IntoIterator<Item=(K, *const u8)>> {
    const NAME: &'static str;

    fn symbols() -> T;
}

pub trait ModuleProvider {
    fn add_functions(&self, builder: &mut JITBuilder, name: impl AsRef<str>);
    fn get_ptrs(&self, hashmap: &mut HashMap<String, *const u8>, name: impl AsRef<str>);
}

pub type DefaultModuleProvider = BorrowingModuleProvider;
pub struct BorrowingModuleProvider;

impl ModuleProvider for BorrowingModuleProvider {
    fn add_functions(&self, builder: &mut JITBuilder, name: impl AsRef<str>) {
        match name.as_ref() {
            "std" => {
                for (name, addr) in Std::symbols() {
                    builder.symbol(name, addr);
                }
            }
            "io" => {
                for (name, addr) in IO::symbols() {
                    builder.symbol(name, addr);
                }
            }
            _ => { /* unknown module */ }
        }
    }

    fn get_ptrs(&self, hashmap: &mut HashMap<String, *const u8>, name: impl AsRef<str>) {
        match name.as_ref() {
            "std" => {
                hashmap.extend(Std::symbols().map(|(a, b)| (a.to_string(), b)));
            }
            "io" => {
                hashmap.extend(IO::symbols().map(|(a, b)| (a.to_string(), b)));
            }
            _ => { /* unknown module */ }
        }
    }
}

struct Std;

#[module(std)]
impl Std {
    fn printc(val: Word) { println!("{val}") }
    fn printa(val: Word) { print!("{}", char::from_u32(val as u32).unwrap()) }
    fn top_8(val: Word) -> Word { val >> 56 }
    fn addr_8(val: Addr) -> Word {
        unsafe {
            let ptr: *mut u8 = std::mem::transmute(val);
            ptr.read() as Word
        }
    }
}

#[repr(transparent)]
struct Wp(Word);

impl Write for Wp {
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut write: Box<dyn Write> = match self.0 {
            0 => { Box::new(std::io::stdout()) }
            1 => { Box::new(std::io::stderr()) }
            2 => { panic!() }
            n => unsafe {
                let file: *mut File = std::mem::transmute(n);

                Box::new(file.read())
            }
        };

        let res = write.write(buf);
        std::mem::forget(write);
        res
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let mut write: Box<dyn Write> = match self.0 {
            0 => { Box::new(std::io::stdout()) }
            1 => { Box::new(std::io::stderr()) }
            2 => { panic!() }
            n => unsafe {
                let file: *mut File = std::mem::transmute(n);

                Box::new(file.read())
            }
        };

        let res = write.flush();
        std::mem::forget(write);
        res
    }
}

impl Read for Wp {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let mut read: Box<dyn Read> = match self.0 {
            0 | 1 => { panic!() }
            2 => { Box::new(std::io::stdin()) }
            n => unsafe {
                let file: *mut File = std::mem::transmute(n);

                Box::new(file.read())
            }
        };
        let res = read.read(buf);
        std::mem::forget(read);
        res
    }
}

struct IO;

#[module(io)]
impl IO {
    fn stdout() -> Word { 0 }
    fn stderr() -> Word { 1 }
    fn stdin() -> Word { 2 }

    fn write(mut handle: Wp, data: Addr, num: Word) {
        unsafe {
            let ptr: *mut u8 = std::mem::transmute(data);
            let slice = std::slice::from_raw_parts_mut(ptr, num as usize);
            handle.write_all(slice).unwrap();
        }
    }

    fn read(handle: Wp, into: Addr, max: Word) -> Word {
        unsafe {
            let ptr: *mut u8 = std::mem::transmute(into);
            let slice = std::slice::from_raw_parts_mut(ptr, max as usize);
            handle.take(max).read(slice).unwrap() as Word
        }
    }

    fn open_file(name: Addr, num: Word) -> Wp {
        unsafe {
            let ptr: *mut u8 = std::mem::transmute(name);
            let slice = std::slice::from_raw_parts_mut(ptr, num as usize);
            let string = String::from_utf8_lossy(slice);

            let file = File::create(string.to_string()).unwrap();
            let mut bx = Box::new(file);

            let ptr = (bx.as_mut() as *mut File) as usize;
            std::mem::forget(bx);
            Wp(ptr as Word)
        }
    }

    fn close_file(file: Wp) {
        unsafe {
            let file: *mut File = std::mem::transmute(file);

            ptr::drop_in_place(file);
        }
    }
}