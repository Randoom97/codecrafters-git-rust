use std::fmt::Display;

use clap::{Parser, Subcommand};

#[derive(Parser, Debug)]
pub struct Args {
    #[clap(subcommand)]
    pub command: Command,
}

#[derive(Subcommand, Debug)]
pub enum Command {
    Init,
    CatFile(CatFileArgs),
}

impl Display for Command {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:?}", self)
    }
}

#[derive(Parser, Debug)]
pub struct CatFileArgs {
    #[arg(short = 'p')]
    pub blob_name: String,
}
