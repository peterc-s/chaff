//! The command-line interface for interacting with the `chaff` anti website fingerprinting
//! framework.

#![expect(missing_docs)]

use bpaf::Bpaf;

/// Command-line interface options.
#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version)]
pub enum CliOptions {
    #[bpaf(command("capture"))]
    /// Capture a traffic trace
    Capture {
        /// Name of the interface to use.
        ifname: Option<String>,
    },
}

/// Dummy docs
fn main() {
    println!("{:?}", cli_options().run());
}
