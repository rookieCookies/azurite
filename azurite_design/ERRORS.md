# Lexial Errors
001) Invalid Character  
- This error occurs if the lexer encounters an unknown character

002) Unterminated String  
- This error occurs while lexer is working on a string and reaches the end of the file

003) Corrupt Unicode Escape  
- This error occurs when encountered a unicode escape sequence without a '{'
    > "\"\\u"

004) Invalid Unicode Value  
- This error occurs while parsing a unicode escape sequence and the value within the brackets is not a valid base-16 character

005) Number Too Large  
- This error occurs while converting a string representation of a number to a 64 bit value. It happens if the string representation can not fit in the 64 bit space

006) Invalid Number For Base
- This error occurs while parsing a number. If the provided characters are not a valid character for the given base

007) Invalid Unicode Character
- This error occurs if the value of the given unicode escape sequences does not map to an existing unicode value

008) Too Many Dots
- This error occurs if a number string has more than 1 dot


# Parser Errors
101) Unexpected Token
- This error occurs when the parser encounters an unexpected token without syntatic structure

102) Unexpected Token
- This error occurs when the parser encounters an unexpected token but unlike `err#101` this comes from syntatic structure

103) Invalid Assignment Value
- This error occurs when you try to update a value which is not one of the following
    - Variable Identifier

104) Invalid data type token
- This error occurs when parsing a data type but encounters a non-datatype token w

105) Invalid statement in a namespace
- This error occurs when there is a statement that is not allowed inside a namespace

106) Invalid namespaced expression
- This error occurs when trying to do something inside a namespace

107) Invalid extern block
- This error occurs when the value after the `extern` keyword isn't a string


# Analysis Errors
201) Invalid Type Arithmetic Operation
- This error occurs when you try to perform an arithmetic operation between invalid types

202) Comparisson types are not the same
- This error occurs when you try to perform a comparisson operation between values of differing types

203) If condition is not a boolean
- This error occurs when the condition of an if expression is not a boolean

204) If expression branches differ in type
- This error occurs when the branches of an if expression return values of differing types

205) Variable does not exist in scope
- This error occurs when mentioning a variable which is not declared yet

206) Can't update a non existent variable
- This error occurs when trying to update a variable which does not exist

207) Variable is of differnet type
- This error occurs when the assigned value and assigned data is of differing types

208) Break outside of loop
- This error occurs when there is a `break` statement outside of a loop

209) Continue outside of loop
- This error occurs when there is a `continue` statement outside of a loop

210) Variable value type differs from the type hint
- This error occurs when the declared variables assigned value differs from the type given

211) Function return value differs
- This error occurs when the functions body returns a value that is not the functions return type

212) Function isn't declared
- This error occurs when trying to call a function that hasn't been declared

213) Function argument is of different type
- This error occurs when calling a function with invalid argument types

214) Type doesn't exist
- This error occurs when a type is used which isn't declared yet

215) Structure isn't declared
- This error occurs when trying to create a structure that isn't declared

216) Structure field doesn't exist
- This error occurs when giving a field to a structure that doesn't have said field

217) Structure field is not of valid type
- This error occurs when the given type and the value is not matching

218) Structure fields invalid
- This error occurs when there's a field on a struct creation but that field does not exist on the struct

219) Structure fields missing
- This error occurs when there's a field missing

220) Structure field doesn't exist
- This error occurs when trying to access a field when the type of the value doesn't have said field

221) Return in main scope
- This error occurs when there is a return statement in the root scope

222) Invalid return type
- This error occurs when the return value is of a different type than the expected type

223) File doesn't exist
- This error occurs when using a file that doesn't exist

224) Invalid Type Order Operation
- This error occurs when you try to perform an order operation between invalid types

225) Invalid Type Unary Operation
- This error occursh when you try to perform a unary operation on a type that doens't support the specific operation

226) Can only cast between primitives
- This error occurs when you try to cast a value by using `as` that is not a primitive

227) Duplicate function definition
- This error occurs when you have functions of the same name inside the same scope

228) Duplicate structure definition
- This error occurs when you have structures of the same name inside the same scope

229) Structure has no generic parameters
230) Structure exists but it has generic parameters
231) Function has no generic parameters
232) Function exists but it has generic parameters