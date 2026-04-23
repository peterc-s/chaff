<div id="user-content-toc" align="center">
  <ul style="list-style: none;">
    <summary>
      <h1 align="center">Chaff</h1>
    </summary>
  </ul>
</div>
<h2 align="center">An experimental website fingerprinting (WF) defence framework</h2>

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="/media/chaff-white.png">
    <img align="center" alt="Chaff Logo" src="/media/chaff-black.png">
  </picture>
</div>

## What is Chaff?

Chaff is a website fingerprinting (WF) defence framework inspired by Maybenot[^1] and the Tor circuit padding framework[^2]. What sets Chaff apart (and makes it experimental) is it's use of _priority queues_. The circuit padding framework is based on Finite State Machines (FSMs), Maybenot is based on extended FSMs (with counters and timers), and Chaff uses a form of queue machine.

### Why experimental?

The reason why Chaff is experimental is because it is unproven. Chaff hasn't been integrated into any existing privacy-enhancing technologies, it is still missing a host of features that would make it complete, and it might not even work that well. One of the key aims for Chaff was to evaluate if the queue machine model is valid and/or useful for developing safe WF defences. 

## Contributing

Chaff is an ongoing dissertation forming part of my ([@peterc-s](https://github.com/peterc-s)) degree. **No contributions are currently being welcomed.** If you are interested in Chaff, wait until around July 2026, when this restriction may be lifted.

## Usage

To build, install `nix` (the package manager) and run:
```sh
nix build
```

To build the documentation, run:
```sh
nix build .#default.doc
```

## Development
Enter the development shell with:
```sh
nix develop
```

Format with:
```sh
nix fmt
```

Run CI locally:
```sh
nix flake check
```

Update flake inputs:
```sh
nix flake update
```

The rest can be done with `cargo` as usual within the development shell.

[^1]: https://github.com/maybenot-io/maybenot/tree/main
[^2]: https://gitlab.torproject.org/tpo/core/tor/-/blob/main/doc/HACKING/CircuitPaddingDevelopment.md
