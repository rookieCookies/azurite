# How to build azurite
To build azurite you'll want to run the `build.py` script inside the
installation directory. This script will compile the azurite offline
installer. After the script's complete you can find the installation
file located at `target/release/azurite_installer` (it might have a
.exe extension if you're on windows). Running this executable will
prompt you with the installer. You can provide the installation dir
to install to.  
Keep in mind you'll have to set up the PATH variable yourself if you
want to use azurite everywhere.  

<br></br>

# Installation Details
After installing inside the installation directory you'll find 3 files
- the executable
- runtime directory
- api directory

## The executable
The name is pretty self-explanatory. It's the executable you'll run
azurite from. It contains a disassembler, the runtime and compiler.

## Runtime Directory
This directory will contain dynamically loaded runtime libraries.
These libraries are necessary for azurite to interact with outside
native code. This folder is hard-coded into the runtime so if the
runtime can not find a library in the current execution directory
it will look here. You can place the libraries which you need globally
here.  
You'll notice there are a few files in there. Those are the built-in 
libraries. Stuff like the standard library etc.

## API Directory
This directory is kind of like the runtime directory except it
contains files for the compiler. This directory is also hard-coded
into the compiler. So it will look here if it can't find a file.  
As per the runtime directory, it already has the built-in library
stuff.