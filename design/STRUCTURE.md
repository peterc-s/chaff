# Structure
The project is structured as follows:
- The root `flake.nix` contains the CI/CD build and testing, it also provides a dev shell that
  provides the same toolchain as the CI/CD. CI/CD pipelines are specified in `.github/workflows`.
- Crates for the different parts of Chaff reside in `crates/`
- The workspace `Cargo.toml` contains project-wide settings and the crates in the `crates/`
  directory must use workspace settings unless explicitly reasoned otherwise.

## Crates
- `chaff` - the main library for Chaff. Contains all the key functionality.
- `chaff-cli` - the CLI which wraps the `chaff` library.
