//! The command-line interface for interacting with the `chaff` anti website fingerprinting
//! framework.

// The derive Bpaf below causes a missing docs lint.
#![expect(missing_docs)]

use bpaf::Bpaf;
use chaff::capture::capture_for_ms;

/// Command-line interface options
#[derive(Debug, Clone, Bpaf)]
#[bpaf(options, version)]
pub enum CliOptions {
    #[bpaf(command("capture"))]
    /// Capture a traffic trace
    Capture {
        /// Name of the interface to use
        #[bpaf(short, long)]
        ifname: Option<String>,
    },
}

/// Dummy docs
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let cli_opts = cli_options().run();
    println!("{cli_opts:?}");

    // Match for subcommands
    match cli_opts {
        CliOptions::Capture { ifname } => {
            println!("Trying with {ifname:?}");
            let cap = capture_for_ms(std::time::Duration::from_secs(10), None)?;
            println!("Captured {} packets.", cap.directions.len());
        }
    }

    Ok(())
}
