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

    fenix = {
      url = "github:nix-community/fenix";
      inputs.nixpkgs.follows = "nixpkgs";
    };
  };

  outputs = {
    self,
    nixpkgs,
    flake-utils,
    treefmt,
    naersk,
    fenix,
  }:
    flake-utils.lib.eachDefaultSystem (
      system: let
        pkgs = nixpkgs.legacyPackages.${system};
        toolchain = fenix.packages.${system}.default.toolchain;
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
          src = ./.;
          buildInputs = with pkgs; [];
        };

        devShells.default = pkgs.mkShell {
          nativeBuildInputs = [
            toolchain
          ];
          RUST_SRC_PATH = "${toolchain}/lib/rustlib/src/rust/library";
        };

        checks = {
          formatting = treefmtConfig.config.build.check self;
          package = self.packages.${system}.default;
          clippy = naersk'.buildPackage {
            src = ./.;
            mode = "clippy";
          };
        };
      }
    );
}
