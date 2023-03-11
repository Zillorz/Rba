# Rba
Rba - really bad assembly

Registers -> A | B | C

Address book -> &index

Instruction Set

Mov -> Moves Value into Register, MOV 2 A;

Swap -> Swaps 2 Registters, SWAP A B;

ADD, SUB, MUL, DIV, MOD. Arthimetic

ADD A 2; | SUB A B; | MUL A &4; | DIV A B; | MOD C 2;

Label, labels a location to jump to, LABEL: loop1;

JZ, jump if zero, JZ 2 loop1;

JNz, jump if not zero, JNZ 2 loop1;

OUT, prints value, OUT 2;
