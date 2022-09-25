# async_aga

## Asynchronous Adaptive Genetic Algorithm

* No generations, no iterations. As soon as resources become available, new individuals are spawned and evaluated. This allows for full exploitation of computational resources, never leaving workers idle waiting for evaluations of others to complete.
* Tree encoding.
* Continuous adaptation of meta parameters, such as mutation probability or mutation width.
* Interactivity via mpsc channels:
  * algorithm sends reports to client
  * client can influence the computation dynamically by sending in command messages, e.g. to change meta parameters
* Well suited to call into from a front end.

Work in progress...
