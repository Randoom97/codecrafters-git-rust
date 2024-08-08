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
    HashObject(HashObjectArgs),
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

#[derive(Parser, Debug)]
pub struct HashObjectArgs {
    #[arg(short = 'w')]
    pub write: bool,
    pub file_path: String,
}
