{
  description = "Anti Website Fingerprinting Library";

  inputs = {
    nixpkgs.url = "github:nixos/nixpkgs?ref=nixos-unstable";
    flake-utils.url = "github:numtide/flake-utils";

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
      in {
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
          package = self.packages.${system}.default;
          clippy = naersk'.buildPackage {
            src = ./.;
            mode = "clippy";
          };
        };
      }
    );
}
