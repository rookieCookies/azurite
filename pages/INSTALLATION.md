# Installation
the recommended way to start using azurite is using the [**pre-compiled binaries**](../downloads "azurite downloads")  
if you're just a user, downloading the [**runtime**](../downloads/runtime/" "azurite downloads -> runtime") should be enough
but for developers   
you will also need the [**compiler**](../downloads/runtime/" "azurite downloads -> compiler")

both the runtime and the compiler are used in the ran same way,  
- the first argument of the command should be the file path
  * if the provided file is a directory, azurite will automatically  
    run every single azurite script in that directory, recursively
- rest of the arguments starting with `--` will be considered environment values
  - example for the compiler: `--release`

for users of azurite, that's it! have fun!
<br></br>
## Actually, getting started, for developers
i could probably write a long paragraph here getting you prepared but  
we will just dive right into the language itself, so create a file  
named "script.az" and put the following text into it
```
IO::write("Hello world!")
```
yep, that's it! now run the following command
```
azurite run 'script.az'
```
and the output should be "Hello world!" just as we said! let's get you  
to [the book](./MAKING_A_GUESSING_GAME.md) shall we?