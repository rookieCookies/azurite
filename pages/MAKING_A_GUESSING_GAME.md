# Making a guessing game
i could explain every single little detail about the language  
but i think seeing it in action might be a bit better :D

make a file named `script.az` and put the following:
```rust
// generate & store a random number
//        between 0 and 100

// get the users input for the guess

// validate and print out feedback based
//        on the users input

```  
the lines that start with `//` are considered comments and they  
are ignored by the compiler, it's kind of like a note for the  
developers! super useful!  

okay realistically we don't really have anything yet so, let's start with  
the first comment. `"generate & store a random number"`  

so first we need to generate a random number, luckily for us azurite  
makes that trivially easy to do and then we can store it into a variable
```rust
using rand

// generate & store a random number
var generated_number = randi()
...
```
variables are defined with the "var" keyword followed by an identifier  
they allow us to store data and access it later on, again, super useful!  

and we import the `randi` function from `rand` that is provided by  
default. you'll notice this is a super common thing, you know importing  
other code.

that was surprisingly easy wasn't it? well we have one issue here, the  
comment under that says that the number should be below 100 but  
`randi()` can give us any number! that's not what want! so let's  
replace it with
```rust
// generate & store a random number
var generated_number = rand_range_int(0, 100)
...
```
and now we have precisely what we want so we can move on to the  
next comment `"get the users input for the guess"`  
shockingly, azurite also makes this trivially easy
```rust
...
// get the users input for the guess
println("please enter a number: ")
var input = read_line()
...
```

notice how we didn't need to import anything this time?  
that's because `read_line` and other common operations are  
included by default.  

so let's move on to the next comment:  
`validate and print out feedback`  
`based on the users input`  
so first we need to check if the users guess was correct and  
if not write out a message telling the user if they are lower/higher  
<br></br>
let's do it. now we won't be able to check the `generated_number`  
directly with the `input` we got, because the `generated_number` is  
an integer while the `input` is a string. so we need a way of converting  
a string to an integer and to do that we call a function to do that for us
```rust
...
// get the users input for the guess
println("please enter a number")
var input = read_line()
var input_as_integer = parse_str_as_int(input)
...  
```
and now we can do math on it!
```rust
...
// validate and print out feedback based
//        on the users input

if input_as_integer == generated_number {
    println("congratz! you were right!")
} else if input_as_integer < generated_number {
    println("sorry! you were too low!")
} else if input_as_integer > generated_number {
    println("sorry! you were too high!")
}
```
...and finally we are done with out guessing game!
congratulations,  you just made your first azurite program!
<br></br>
our final code looks like this

```rust
using rand

// generate & store a random number
var generated_number = rand_range_int(0, 100)

// get the users input for the guess
println("please enter a number")
var input = read_line()
var input_as_integer = parse_str_as_int(input)

// validate and print out feedback based
//        on the users input

if (input_as_integer == generated_number) {
    println("congratz! you were right!")
} else if input_as_integer < generated_number {
    println("sorry! you were too low!")
} else if input_as_integer > generated_number {
    println("sorry! you were too high!")
}
```


# Challenges
## Challenge 1)
- make it so the user has infinite tries. make sure the `secret_number` isn't changed
    between tries. only exit if the user guesses correctly
<details>
<summary>
<strong>Solution</strong>
</summary>

```rust
using rand

// generate & store a random number
var generated_number = rand_range_int(0, 100)

loop {
    // get the users input for the guess
    println("please enter a number")
    var input = read_line()
    var input_as_integer = parse_str_as_int(input)

    // validate and print out feedback based
    //        on the users input
    
    if (input_as_integer == generated_number) {
        println("congratz! you were right!")
        break
    } else if input_as_integer < generated_number {
        println("sorry! you were too low!")
    } else if input_as_integer > generated_number {
        println("sorry! you were too high!")
    }
}
```

`explanation:` azurite provides a built-in keyword for infinite loops. we put everything
except the line we generate the random number inside an infinite loop so until we explicitly break out of it it will keep repeating the same thing over and over. when we get the exact same number as the generated number we `break` out of the loop and exit the program  
<br>
`note:` using a `while true` is also a valid way to solve this challenge but it's more idiomatic to use the `loop` keyword instead of `while true` in azurite
</br>
</details>