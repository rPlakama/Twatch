# How does it works? 

- Twatch currently works by capturing HWMON devices using Rust and creates a file, which Python does create an plot.


# Graphic Examples:

#![First](./examples/1.png) 
#![Second](./examples/2.png) 

Firstly: 

- Making session selector (Currently there are the session printer, easy tho)
- Making file be saved as a buffer (Struct) to be reused as treated by other functions (The treated data, X and Y to be used by the drawing in GTK)
- Preparing the plot table, containing the X scaled by the amount of captures and y as an fixed value (0 to 110C)

Secondary:

- Hybrid POSIX flags 
- Making show-hidden-posix (smth in line 70 or whatever)
- Creating a selection list for sessions [0%]
- Power usage monitor
- More detailed graphs
- extensive selection for devices

# How can I run it? 

Currently, the project is based on Nix, which you can have a temporary shell with this command:

```nix shell github:rPlakama/Twatch``` 

But, since it uses small depedencies (pandas, and matplotlib). You can copy this repo with a mere clone. 

(Since the development flake paramenters, the main.rs won't be able to pull the graph. For testing, you need to execute ```python graph.py``` in your shell.)

# If I want to Colaborate? 

In case that you want to help this project (Which sucks) feels free, I would be glad! Make a fork and create an pull request.
