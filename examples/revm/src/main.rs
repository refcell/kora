#![doc = include_str!("../README.md")]
#![doc(issue_tracker_base_url = "https://github.com/refcell/kora/issues/")]
#![cfg_attr(docsrs, feature(doc_cfg, doc_auto_cfg))]
#![cfg_attr(not(test), warn(unused_crate_dependencies))]

mod app;
mod cli;
mod handle;
mod outcome;
mod runner;
mod simulation;

fn main() {
    use clap::Parser;

    let cli = cli::Cli::parse();

    let outcome = cli.run();
    if outcome.is_err() {
        eprintln!("Simulation failed: {:?}", outcome);
        std::process::exit(1);
    };

    let outcome = outcome.expect("success");
    println!("{}", outcome);
}
