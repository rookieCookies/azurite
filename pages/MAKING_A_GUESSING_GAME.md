# Making a guessing game
woah, slow down there we just got started. I'm not going to sit  
here and explain to you every little detail before we even made  
something so let's get started!  

you should already have a `script.az` file, if not create one and  
write the following text in there
```
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
```
using "std_rand"

// generate & store a random number
var generated_number = rand_int()
...
```
variables are defined with the "var" keyword followed by an identifier  
they allow us to store data and access it later on, again, super useful!  

that was surprisingly easy wasn't it? well we have one issue here, the  
comment under that says that the number should be below 100 but  
`rand_int()` can give us any number! that's not what want! so let's  
replace it with
```
using "std_rand"

// generate & store a random number
var generated_number = rand_range(0, 100)
...
```
and now we have precisely what we want so we can move on to the  
next comment `"get the users input for the guess"`  
shockingly, azurite also makes this trivially easy
```
...
// get the users input for the guess
var input = IO::read()
...
```
okay, I know that looks a bit confusing so let's break it down  
  
in azurite you can access a types static functions (which we  
will get to later) by typing the type's name and then two colons  
and in here we are calling the read() function of the type IO  
  
so let's move on to the next comment  
`validate and print out feedback`  
`based on the users input`  
so first we need to check if the users guess was correct and  
if not write out a message telling the user if they are lower/higher  
<br></br>
let's do it. now we won't be able to check the `generated_number`  
directly with the `input` we got, because the `generated_number` is  
an integer while the `input` is a string. so we need a way of converting  
a string to an integer and to do that we call a static function on the type  
integer
```
...
// get the users input for the guess
var input = IO::read()
var input_as_integer = int::parse_str(input)
...  
```
and now we can do math on it!
```
...
// validate and print out feedback based
//        on the users input

if input_as_integer == generated_number {
    IO::write("congratz! you were right!")
} else if input_as_integer < generated_number {
    IO::write("sorry! you were too low!")
} else if input_as_integer > generated_number {
    IO::write("sorry! you were too high!")
}
```
as you might have noticed we are calling another static function on  
the type IO, the IO::write function allows us to write something to  
the console without much hassle  
<br></br>
...and finally we are done with out guessing game! congratulations,  
you just made your first azurite program!
<br></br>
our final code looks like this

```
using "std_rand"

// generate & store a random number
var generated_number = rand_int()

// get the users input for the guess
var input = IO::read()
var input_as_integer = int::parse_str(input)

// validate and print out feedback based
//        on the users input

if input_as_integer == generated_number {
    IO::write("congratz! you were right!")
} else if input_as_integer < generated_number {
    IO::write("sorry! you were too low!")
} else if input_as_integer > generated_number {
    IO::write("sorry! you were too high!")
}
```
## Challanges
> It's okay if you can't do it, these challanges require more  
> knowledge and syntax than we have learned in this page, try  
> to come back to them after a bit!  

<br>
<strong>Challange 2</strong>: Try to make it so the user can continue after one  
guess and the generated number stays the same  
</br>
<br>
<strong>Challange 2</strong>: If the user enters an invalid number the program crashes,  
can you find a way to fix that?
</br>

<br>
<details>
    <summary> <strong>Solution 1</strong> </summary>
        
    using "std_rand"

    // generate & store a random number
    var generated_number = rand_int()
    while true {
        // get the users input for the guess
        var input = IO::read()
        var input_as_integer = int::parse_str(input)

        // validate and print out feedback based
        //        on the users input

        if input_as_integer == generated_number {
            IO::write("congratz! you were right!")
            break
        } else if input_as_integer < generated_number {
            IO::write("sorry! you were too low!")
        } else if input_as_integer > generated_number {
            IO::write("sorry! you were too high!")
        }
    }
</details>

<details>
    <summary> <strong>Solution 2</strong> </summary>
        
    using "std_rand"

    // generate & store a random number
    var generated_number = rand_int()

    // get the users input for the guess
    var input = IO::read()
    var input_as_integer = try {
        int::parse_str(input)
    } catch {
        IO::write("please provide a valid integer")
        Runtime::exit()
    }

    // validate and print out feedback based
    //        on the users input

    if input_as_integer == generated_number {
        IO::write("congratz! you were right!")
    } else if input_as_integer < generated_number {
        IO::write("sorry! you were too low!")
    } else if input_as_integer > generated_number {
        IO::write("sorry! you were too high!")
    }
</details>
</br>