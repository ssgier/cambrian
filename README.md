# cambrian

Project in early stage, work in progress... 

### Asynchronous Adaptive Genetic Algorithm

* No generations and no iterations. As soon as workers become available, new individuals are spawned and evaluated reactively. This allows for full exploitation of computational resources, never leaving workers idle waiting for others to complete an evaluation.
* No configuration by the user needed apart from the objective function definition. Meta parameters (crossover probability, selection pressure, mutation probability and width) adapt dynamically to the problem by co-evolving.
* Highly expressive objective function parameter specification (parameter names, data types, bounds, nesting) via YAML file. Various data types are supported:
  * boolean
  * integer
  * real numbers
  * optionals (recursive)
  * variants (recursive)
  * enums
  * collections (recursive, resizable via crossover and mutation)
* Objective function implementation can be done in any programming language. It is passed to the cambrian app in form of a command line program that accepts the function parameters as an argument formatted in JSON. Cambrian will then call into it by forking child processes. Both sequential and parallel execution are supported. (Not suitable for very short-lived objective functions)
