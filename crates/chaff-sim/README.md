# Chaff Simulator

See documentation and tests for usage.

## Benchmarks

To run benchmarks, you must first download the `Undefended.zip` dataset from [here](https://zenodo.org/records/11631265), unzip it to `WORKSPACE_ROOT/data/tik_tok_undefended`[^1], then use the `chaff-cli` to convert it to chaff dataset format with the `dataset-convert` subcommand into `WORKSPACE_ROOT/data/tik_tok_undefended.chaff`.

Then, you can run `cargo bench` to run all benchmarks.

[^1]: The `WORKSPACE_ROOT` is where the main `README.md` lives.
