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
        src = builtins.path {
          path = ./.;
          name = "chaff";
        };

        # Toolchain for development use
        toolchain = pkgs.rust-bin.stable.latest.default.override {
          extensions = ["rust-analyzer" "rust-src"];
        };
        naersk' = naersk.lib.${system}.override {
          cargo = toolchain;
          rustc = toolchain;
        };

        # Toolchain for MSRV checking. MSRV is stated here and:
        # - in the README
        # - in the workspace Cargo.toml
        # This should follow Arti.
        toolchainMsrv = pkgs.rust-bin.stable."1.86.0".default.override {
          extensions = ["rust-analyzer" "rust-src"];
        };
        naerskMsrv = naersk.lib.${system}.override {
          cargo = toolchainMsrv;
          rustc = toolchainMsrv;
        };

        treefmtConfig = treefmt.lib.evalModule pkgs {
          projectRootFile = "flake.nix";
          programs.rustfmt.enable = true;
          programs.alejandra.enable = true;
        };

        buildInputs = with pkgs; [libpcap];
        extraShellInputs = with pkgs; [cargo-audit cargo-nextest cargo-machete cargo-tarpaulin];
      in {
        formatter = treefmtConfig.config.build.wrapper;

        # Default package for `nix build`
        packages.default = naersk'.buildPackage {
          inherit buildInputs src;
          doDoc = true;
        };

        devShells = {
          # Default development shell
          default = pkgs.mkShell {
            buildInputs = buildInputs ++ extraShellInputs;
            nativeBuildInputs = [
              toolchain
            ];
            RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
          };

          # MSRV development shell
          msrv = pkgs.mkShell {
            buildInputs = buildInputs ++ extraShellInputs;
            nativeBuildInputs = [
              toolchainMsrv
            ];
            RUST_SRC_PATH = "${toolchainMsrv}/lib/rustlib/src/rust/library";
          };
        };

        checks = {
          # This will just fail in CI/CD, but locally will actually
          # do the formatting.
          formatting = treefmtConfig.config.build.check self;

          # In CI/CD this will already be fulfilled as we build the package
          # with `nix build` beforehand
          package = self.packages.${system}.default;

          # Do a check with the toolchain set at the MSRV
          msrv = naerskMsrv.buildPackage {
            inherit buildInputs src;
            mode = "check";
          };

          # Lint and test
          clippy = naersk'.buildPackage {
            inherit buildInputs src;
            mode = "clippy";
          };
          test = naersk'.buildPackage {
            inherit buildInputs src;
            mode = "test";
          };
        };
      }
    );
}
