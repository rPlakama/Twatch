# How does it works? 

- Twatch currently works by capturing HWMON devices using Rust and creates a file, which Python does create an plot.


# Graphic Examples:

#![First](./examples/1.png) 
#![Second](./examples/2.png) 


TODO:

- Watts monitor [ Currently able to capture and format to watts ]
-> Needs to add session writting for this type of collection.
-> Needs to create an way to python reconize POSIX flags to the command can be executed aproperly based on the type of session
-> System to exclude or add devices based also on POSIX flags for Python (That can managed with the Rust execution, easy)


Secondary:

- Creating a selection list for sessions [0%]
- Power usage monitor
- More detailed graphs
- extensive selection for devices

# How can I run it? 

Currently, the project is based on Nix, which you can have a temporary shell with this command:

```nix shell github:rPlakama/Twatch``` 

But, since it uses small depedencies (pandas, and matplotlib). You can copy this repo with a mere clone. 


# If I want to Colaborate? 

In case that you want to help this project (Which sucks) feels free, I would be glad! Make a fork and create an pull request.
