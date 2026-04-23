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

## Building
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
