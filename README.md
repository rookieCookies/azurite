# azurite
<img align="right" width="280px" height="250px" src="https://cdn.discordapp.com/attachments/1098266476361818254/1114236173410386090/image.png">  

**azurite is an easy to use programming language allowing developers to start making the stuff they want without any hassle**


# why? 
azurite allows for a tidy and concise writing experience with syntax that should be familiar to anyone with any experience in languages like C, Rust, C++, Java, etc. and if you don't, I'm sure you'll be able to pick it up in no time and love it! Or not, I'm not your mum


# a small taste of azurite
let's look at the most basic example! the good ol' hello world
```
println("hello world!")
```
oh. that's it?  
<br>
okay maybe let's look at a more complicated example, how about.. a guessing game?
```
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
maybe a bit too much? don't worry [i gotchu](./pages/MAKING_A_GUESSING_GAME.md)

# todo:
* DONE multi-file support 
* make it so users can use a command like "azurite" instead of  
  running the executable directly, preferably make it automaticly  
  update the path value when installing
* generics
* tuples
* traits
* ranges
* enums
* match statements
* more tests
* more standard library functionality
* tools
  * command line interface
  * library manager
  * code formatter
