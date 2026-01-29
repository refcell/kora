use clap::Parser;

use crate::{outcome::SimOutcome, simulation::simulate};

#[derive(Clone, Copy, Debug)]
pub(crate) struct SimConfig {
    pub(crate) nodes: usize,
    pub(crate) blocks: u64,
    pub(crate) seed: u64,
}

#[derive(Parser, Debug)]
#[command(name = "kora-revm-example")]
#[command(about = "threshold-simplex + EVM execution example")]
pub(crate) struct Cli {
    #[arg(long, default_value = "4")]
    pub nodes: usize,

    #[arg(long, default_value = "3")]
    pub blocks: u64,

    #[arg(long, default_value = "1")]
    pub seed: u64,
}

impl Cli {
    pub(crate) fn run(self) -> anyhow::Result<SimOutcome> {
        simulate(SimConfig { nodes: self.nodes, blocks: self.blocks, seed: self.seed })
    }
}
