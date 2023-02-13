# cambrian

## A Pragmatic Global Black-Box Optimizer

- Cambrian is a tool for global black-box optimization. Its optimization algorithm falls under the category of **Adaptive Genetic Algorithms**. User documentation can be found [here](https://github.com/ssgier/cambrian/wiki).

- Comes in form of an **out-of-the-box command line application**. No configuration is needed apart from the objective function definition.
  
- The objective function implementation is provided by the user in form of a stand-alone program that reads and writes JSON. Usage is therefore **programming language agnostic.**
  
- The parameter space is provided by the user in form of a YAML file. The format is **highly expressive**: It can represent parameter spaces ranging from simple real-valued vectors to **hierarchical data structures with resizing parts.**
  
- Support for parallelism.
  
- Linux is currently the only supported OS.
  

### Example Use Cases:

- Optimize the topology and other higher level parameters of a deep neural network. This includes optimizing across different numbers and types of layers, each with their specific sub-parameters.
  
- Optimize the runtime performance of a data processing application. This could include optimizing parameters like chunk sizes, numbers of threads, and flags like those passed to a compiler or a VM.
  

### Trivial Example for Illustration

> **Note:** the objective function used in this example is very short running, has only two dimensions and the analytical solution is known. It is suitable for illustration purposes only.

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
`scale` represents the order of magnitude of variation. It can be thought of as something similar to the standard deviation of the result when the value is mutated. It is merely a hint to the mutation logic and can be provided by the user intuitively.


Our objective function program can be written in any programming language. Here we choose Python. Cambrian will call the program as a child process and pass the parameters in form of a JSON as the last argument. If our script was called `obj_func.py`, then cambrian would start it with a call equivalent to the following command:

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

And finally, this is how the usage of cambrian would look like in the terminal, including output:

```
$ cambrian -s spec.yaml python obj_func.py -t 1e-3
{"x":0.0002100776985471467,"y":-0.00013167246263939315}
```

Here the `-t` option is an instruction to terminate as soon as an objective function value of 1e-3 is reached. Several kinds of termination criteria are available (see [Command Line Usage](https://github.com/ssgier/cambrian/wiki/Command-Line-Usage) for more details), and it is always possible to terminate manually by hitting Ctrl-C (or sending SIGINT), which will instruct cambrian to terminate gracefully and yield the best seen individual.


### Installation

There are three ways to install cambrian:
* build from source
* install from crates.io
* download from GitHub

Building from source or installing from crates.io requires Cargo (the Rust package manager and build system) to be installed. For installing Cargo itself, please see [Cargo Installation](https://doc.rust-lang.org/cargo/getting-started/installation.html)

#### Build from Source
Clone the repository from GitHub, then build and install it using the following sequence of commands:

```
git clone git@github.com:ssgier/cambrian.git
cd cambrian
cargo build --release
cargo install --path .
```

After running these commands, cambrian should have been installed to `~/.cargo/bin`. Add the directory to the PATH variable if needed.

#### Install from crates.io
```
cargo install cambrian
```

#### Download from GitHub
Go to [releases](https://github.com/ssgier/cambrian/releases) and download the latest archive (cambrian-v0.2.0-x86_64-unknown-linux-musl.tar.gz). Extract it:
```
tar xvzf cambrian-v0.2.0-x86_64-unknown-linux-musl.tar.gz
```
This will extract a directory containing the cambrian executable. Place the executable in a directory of choice and optionally add that directory to the PATH environment variable.

### Documentation

See the [cambrian wiki](https://github.com/ssgier/cambrian/wiki) for an in-depth tutorial. To get an overview of the available data types, see [Specs and Values](https://github.com/ssgier/cambrian/wiki/Specs-and-Values).
