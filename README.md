# Chaff
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
