import os
import shutil

cwd = os.getcwd()

print("Compiling CLI Tool")
os.system("cargo build --release --bin=azurite_cli")
print("Compiled CLI Tool")

# Compile built in libraries
print("Compiling builtin libraries")
os.chdir("builtin_libraries")

if os.path.exists(".target"):
    os.rename(".target", "target")

os.system("cargo build --release")

if not os.path.exists("azurite_libraries"):
    os.mkdir("azurite_libraries")

release_folder = os.getcwd() + "/target/release/"
library_folder = os.getcwd() + "/azurite_libraries/"
for filename in os.listdir(release_folder):
    if filename.endswith(".dll") or filename.endswith(".so") or filename.endswith("dylib"):
        to_file = filename
        if filename.endswith(".so") or filename.endswith("dylib"):
            to_file = filename[3:]
        shutil.copy(release_folder + filename, library_folder + to_file)
        continue


if os.path.exists("target"):
    os.rename("target", ".target")

os.chdir(cwd)
print("Compiled builtin libraries")


# Compile installer
print("Compiling installer")
os.system("cargo build --release --bin=azurite_installer")
print("Compiled installer")
