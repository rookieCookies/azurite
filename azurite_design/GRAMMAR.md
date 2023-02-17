statement:  
|> variable-declaration  
|> variable-update  
|> while-statement  
|> function-declaration  
|> return-statement  
|> structure-declaration  
|> assert-statement  
|> impl-block  
|> raw-call  
|> expression  

variable-declaration:  
|> 'var' identifier (':' type)? '=' expression  

variable-update:  
|> identifier '=' expr  
|> identifier( '.' identifier )* '=' expr  

while-statement:  
|> 'while' comparison-expression body  

function-declaration:  
|> 'fn' identifier '(' [identifier : type]* ')' body  
|> 'fn' identifier '(' [identifier : type]* ')' '->' type body  

return-statement:  
|> 'return' expression  

structure-declaration:  
|> 'struct' identifier '{' [identifier ':' type ',']* '}'  

impl-block:  
|> 'impl' identifier '{' function-declaration* '}'  

raw-call:  
|> 'raw' INTEGER  

using-statement:  
|> 'using' STRING  

expression:  
|> comparison-expression  

comparison-expression:  
|> not-operation  
|> arithmetic-expression '=='|'!='|'>='|'<='|'>'|'<' arithmetic-expression  

arithmetic-expression:  
|> product-expression '+'|'-' product-expression  

product-expression:  
|> factor-expression '*'|'/' factor-expression  

factor-expression:  
|> negation-expression  
|> power-expression  

power-expression:  
|> unit '^' factor-expression  

unit:  
|> atom  
|> unit ( '.' identifier )*  
|> unit ( '.' function-call )*  

atom:  
|> INTEGER  
|> FLOAT  
|> STRING  
|> body  
|> variable-access  
|> if-expression  
|> function-call  
|> structure-creation  
|> '(' expression ')'  

body:  
|> '{' statement* '}'  

variable-access:  
|> identifier  

if-expression:  
|> 'if' comparison-expression body  
|> 'if' comparison-expression body 'else' if-expression  
|> 'if' comparison-expression body 'else' body  

function-call:  
|> identifier '(' expression* ')'  

structure-creation:  
|> identifier '{' [identifier ':' expression ',']* '}'  

not-operation:  
|> '!' comparison-expression  

negation-operation:  
|> '-' factor-expression  
