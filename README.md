# cambrian

Work in progress. First release and full documentation will be available soon...

## A Pragmatic Global Black-Box Optimizer

- **Out-of-the-box:** No configuration needed apart from the objective function definition
  
- Objective function is provided by the user as a command line program that reads and writes JSON. Can be implemented in any programming language
  
- **Highly expressive**: Can represent various kinds of parameter sets, ranging from simple real-valued vectors to **hierarchical data structures with resizing parts**
  
- Support for parallelism
  
- Implemented as an adaptive genetic algorithm
  
- Currently Linux is the only supported OS
  

### Example Use Cases:

- Optimize the topology and other higher level parameters of a deep neural network. This includes optimizing across different numbers and types of layers, each with their specific sub-parameters.
  
- Optimize the runtime performance of a data processing application. This could include optimizing parameters like chunk sizes, numbers of threads, and flags like those passed to a compiler or a VM.
  

### Trivial Example for Illustration

> **Note:** the objective function used in this example is very short running, has only two dimensions, and the analytical solution is known. It is suitable for illustration purposes only and not the intended use case for this software.

Let's say we wanted to optimize the two-dimensional function f(x, y) = x<sup>2</sup> + y<sup>2</sup>. We have to provide a spec file in YAML and a command line program that implements the function. For illustration purposes, let's pretend we didn't know the real solution, but we know that y will be no greater than 1.5. We decide that our initial guess is x = y = 1.0. This is how our spec file (let's call it `spec.yaml`) would look like:

```
x:
    type: real
    init: 1.0
    scale: 0.1
y:
    type: real
    init: 1.0
    scale: 0.1
    max: 1.5
```

Our objective function program can be written in any programming language. Here we choose Python. Cambrian will call the program as a child process and pass the parameters in form of a JSON as the last argument. So if our script was called `obj_func.py`, then cambrian would start our process with a call equivalent to the following command:

```
python obj_func.py '{"x":1.0,"y":1.0}'
```

and it would expect the program to print a line in the following format to the standard output:

```
{"objFuncVal": 2.0}
```

The script `obj_func.py` itself could look like this:

```
import sys
import json

data = json.loads(sys.argv[1])
x = data['x']
y = data['y']

result = {
    "objFuncVal": x * x + y * y
}

print(json.dumps(result))
```

And finally, this is how the usage on the terminal would look like, including output:

```
$ cambrian -s spec.yaml python obj_func.py -t 1e-3
{"x":0.0002100776985471467,"y":-0.00013167246263939315}
```

Here the `-t` option is an instruction to terminate as soon as an objective function value of 1e-3 is reached. Several kinds of termination criteria are available (see `cambrian --help`), and it is always possible to terminate manually by hitting Ctrl-C (or sending SIGINT), which will instruct cambrian to terminate gracefully and yield the best seen candidate.

### Installation

Binary packages will be provided soon, but currently the only way to install cambrian is to build it from source using Cargo (the Rust package manager and build system). For installing Cargo itself, please see:

[Installation - The Cargo Book](https://doc.rust-lang.org/cargo/getting-started/installation.html)

Once Cargo is installed, clone the repository from GitHub, then build and install it using the following sequence of commands:

```
git clone git@github.com:ssgier/cambrian.git
cd cambrian
cargo build --release
cargo install --path .
```

After running these commands, cambrian should have been installed to `~/.cargo/bin`. Add the directory to the PATH variable if needed.

### Documentation and Advanced Use Case Examples

Work in progress...
