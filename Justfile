default: build

build:
    nix build

test: build
    nix flake check

fmt:
    nix fmt

update:
    nix flake update
