#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/kora/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod application;
mod cli;
mod config;
mod demo;
mod outcome;
mod simulation;
mod tx;

fn main() {
    use clap::Parser;

    // Parse the cli.
    let cli = cli::Cli::parse();

    // Run the simulation.
    let outcome = cli.run();
    if outcome.is_err() {
        eprintln!("Simulation failed: {:?}", outcome);
        std::process::exit(1);
    };

    // Print the output.
    let outcome = outcome.expect("success");
    println!("{}", outcome);
}
