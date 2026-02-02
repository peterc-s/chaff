{
  description = "Anti Website Fingerprinting Library";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";
    treefmt.url = "github:numtide/treefmt-nix";

    naersk = {
      url = "github:nix-community/naersk";
      inputs.nixpkgs.follows = "nixpkgs";
    };

    rust-overlay = {
      url = "github:oxalica/rust-overlay";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    treefmt,
    naersk,
    rust-overlay,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        overlays = [(import rust-overlay)];
        pkgs = import nixpkgs {
          inherit system overlays;
        };
        toolchain = pkgs.rust-bin.stable."1.86.0".default;
        naersk' = naersk.lib.${system}.override {
          cargo = toolchain;
          rustc = toolchain;
        };
        treefmtConfig = treefmt.lib.evalModule pkgs {
          projectRootFile = "flake.nix";
          programs.rustfmt.enable = true;
          programs.alejandra.enable = true;
        };
      in {
        formatter = treefmtConfig.config.build.wrapper;

        packages.default = naersk'.buildPackage {
          src = builtins.path {
            path = ./.;
            name = "chaff";
          };
          buildInputs = with pkgs; [libpcap];
          doDoc = true;
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = [
            toolchain
          ];
          buildInputs = with pkgs; [libpcap];
          RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
        };

        checks = {
          formatting = treefmtConfig.config.build.check self;
          package = self.packages.${system}.default;
          clippy = naersk'.buildPackage {
            src = builtins.path {
              path = ./.;
              name = "chaff";
            };
            buildInputs = with pkgs; [libpcap];
            mode = "clippy";
          };
          test = naersk'.buildPackage {
            src = builtins.path {
              path = ./.;
              name = "chaff";
            };
            buildInputs = with pkgs; [libpcap];
            mode = "test";
          };
        };
      }
    );
}
