<div id="user-content-toc" align="center">
  <ul style="list-style: none;">
    <summary>
      <h1 align="center">Chaff</h1>
    </summary>
  </ul>
</div>
<h2 align="center">An experimental website fingerprinting (WF) defence framework.</h2>

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="/media/chaff-white.png">
    <img align="center" alt="Chaff Logo" src="/media/chaff-black.png">
  </picture>
</div>
<p align="center">
  <i>Chaff's logo, the effect of <a href="https://en.wikipedia.org/wiki/Chaff_(countermeasure)">chaff</a> on the display of a Würzburg Riese radar, edited to form a "C".<br><a href="https://commons.wikimedia.org/wiki/File:Giant_Wurzburg_Display_-_Window_Effect.jpg">Original image source</a>.</i>
</p>

## What is Chaff?

Chaff is a website fingerprinting (WF) defence framework inspired by Maybenot[^1] and the Tor circuit padding framework[^2]. What sets Chaff apart (and makes it experimental) is it's use of _priority queues_. The circuit padding framework is based on Finite State Machines (FSMs), Maybenot is based on extended FSMs (with counters and timers), and Chaff uses a form of queue machine.

### Why experimental?

The reason why Chaff is experimental is because it is unproven. Chaff hasn't been integrated into any existing privacy-enhancing technologies, it is still missing a host of features that would make it complete, and your mileage may vary. One of the key aims for Chaff was to evaluate if the queue machine model is valid and/or useful for developing safe WF defences. 

### Why Chaff?

WF defences often act like countermeasures to detection that confuse a classifier. [Chaff](https://en.wikipedia.org/wiki/Chaff_(countermeasure)) is a countermeasure for RADAR that confuses an operator.

## Contributing

This is an ongoing dissertation forming part of my ([@peterc-s](https://github.com/peterc-s)) degree. **No contributions are currently being welcomed.** If you are interested in Chaff, wait until around July 2026, when this restriction may be lifted.

## Usage

Chaff consists of many crates, each serving their own purpose:

- `chaff` - the main framework library. Contains the entrypoint (the `Framework` `struct`) into Chaff along with all of its component parts.
- `chaff-sim` - the simulator for running Chaff defences on traces.
- `chaff-capture` - a basic trace capture library, built on top of [`pcap`](https://github.com/rust-pcap/pcap) which is in turn built on top of [`libpcap`](https://github.com/the-tcpdump-group/libpcap).
- `chaff-datasets` - parsers for different WF papers' datasets.
- `chaff-machines` - defences implemented in Chaff.
- `chaff-cli` - a basic CLI for using the various Chaff features.

See their respective `README.md`s and documentation for specific usage.

## Building

For reproducible builds, Chaff uses [`nix`](https://nixos.org/download/). A `flake.nix` is provided with all the development tools needed and a couple of development shells.

If you just want to build Chaff, you can run:

```sh
nix build
```

The resulting artefacts will be put in `result/`.

If you don't want to use Nix, you can use `cargo` directly, but you may need external libraries such as `libpcap`:

```sh
cargo build --release
```

To use the command line:

```sh
cargo run --release -- --help
```

To install:

```sh
cargo install --path ./crates/chaff-cli/

# then
chaff-cli --help
```

## Development

For development purposes, you might want to enter the default development shell:

```sh
nix develop
```

You can now use `cargo` to build Chaff just like any other Rust project.

To check changes before making a PR, you can run:

```sh
nix flake check
```

Which is what the CI/CD pipeline does.

> [!NOTE]
> CI/CD can still fail even if `nix flake check` passes locally. This usually happens if the flake inputs are out of date (run `nix flake update`) or if [Arti](https://gitlab.torproject.org/tpo/core/arti) has bumped their MSRV.

Chaff uses aggressive `clippy` lints and should compile with `-Wpedantic`. It also aims for 100% coverage via [`cargo-tarpaulin`](https://github.com/xd009642/tarpaulin) using the LLVM backend. There are some false positives, currently they are not being ignored with attributes: if code changes within the scope of the attribute and is no longer covered, you couldn't tell.

## Datasets

By convention, datasets should go in `data/`. See [`chaff-datasets`](./crates/chaff-datasets/README.md) and [`chaff-sim`](./crates/chaff-sim/README.md) for information on acquiring the Tik-Tok dataset.

[^1]: https://github.com/maybenot-io/maybenot/tree/main
[^2]: https://gitlab.torproject.org/tpo/core/tor/-/blob/main/doc/HACKING/CircuitPaddingDevelopment.md
