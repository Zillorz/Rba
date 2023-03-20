# Rba
Rba - really bad assembly

Assembly? Honestly the string and module system are pushing it past most ASM languages and this is becoming closer to a simple C

# Programming
The basics on writing a program in this language.

## Basics
A **variable** is denoted with a character, all variables exist in a program, but are initalized to 0

All types are unsigned 64 bit ints (u64)

The & Operator gets the u64 value at an address: 
For example `OUT &100` will output the u64 at the 100th memory address

2 types of constants
1. Simple constants, a u64 number, can have underscores inbetween `111_222_333`
2. String constant, is the address of the string, `"Hello World!"`, reminder: parsed as json string, does support `\n` and other escape sequences

Labels, any text which does not contain any of the following `",\;` 

VAR - Is variable or memory address, can be set to

VAL - Is variable or constant

## Instructions
1) INC \<LABEL\>; adds set of functions from module
2) MOV \<VAL\> \<VAR\>; sets variable to value
3) SWAP \<VAR\> \<VAR\>; swaps 2 variables, (may be removed)
4) ADD/SUB/MUL/DIV/MOD \<VAR\> \<VAL\>; preforms math operation, result stored in variable
5) LABEL: \<LABEL\>; Represents a point which can be jumped to, can have the same name as module, does not interfere
6) JZ/JNZ \<VAL\> \<LABEL\>; jumps to label if val is 0 (JZ) or not 0 (JNZ)
7) CALL \<LABEL\> \<VAL\>, \<VAL\> ... \<VAR?\>; Calls function from module, comma seperated list of agruments, last variable is return value
8) RCALL \<LABEL\> <VAR?>; Calls function with no arguments, (parser limitation (can be fixed))
9) OUT \<VAL\>; Prints value as u64;
10) NOP; does nothing

## Modules
There are currently 2 simple modules in rba.
Modules are one of the non-assmebly like features in the language

### IO
All writeable objects have a u64 handle

Included with `INC io;`

Handle for stdout, stderr and stdin and 0, 1, and 2 respectively

**Functions**
1. `CALL write handle, ptr, num;` writes `num` bytes from `ptr` into `handle`
2. `CALL read handle, ptr, num am;` reads at max `num` bytes from handle and puts them into `ptr`, returns amount of bytes read (into `am`)
3. `CALL open_file ptr, num handle;` takes `num` bytes from `ptr`, parses them into utf8 bytes, and returns (writes into `handle`) file handle with that name
4. `CALL close_file handle;` closes file handle stored in `handle`
5. `RCALL stdout handle` `RCALL stdin handle` `RCALL stderr handle`, writes stdin, stderr, or stdout handle to `handle`

### STD
Simple (useless/debugging) methods

Included with `INC std;`

**Functions**
1. `CALL printc val;` does the same thing as `OUT val;` can be called without std
2. `CALL printa val;` reads `val`, converts to utf8 character, and prints to console
3. `CALL addr_8 addr out;` reads u8 from `addr` and returns (writes to `out`) it
4. `CALL top_8 val out;` shifts `val` right 56 bits and returns (writes to `out`) it


### C functions
All C functions are also supported

Notable functions
1. `CALL malloc 1024 Z;` returns (writes to `Z`) 1024 byte pointer
