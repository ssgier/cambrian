# async_aga
## Asynchronous Adaptive Genetic Algorithm
* No generations, no iterations. As soon as resources become available, new individuals are spawned and evaluated. This allows for full exploitation of computational resources, never leaving threads idle waiting for evaluations of others to complete.
* Supports boolean, integer, and real-valued parameters.
* Continuous adaptation of meta parameters, such as mutation probability or mutation width.
* Interactivity via mpsc channels:
  * algorithm sends reports to client
  * client can influence the computation dynamically by sending in command messages, e.g. to change meta parameters
