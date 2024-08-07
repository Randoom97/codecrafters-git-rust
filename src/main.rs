use arg_parse::{Args, Command};
use clap::Parser;

mod arg_parse;
mod commands;
mod git_object;
mod reader_utils;

fn main() {
    let args = Args::parse();
    match args.command {
        Command::Init => {
            let result = commands::init();
            if result.is_ok() {
                println!("Initialized git directory");
            } else {
                eprintln!("{}", result.unwrap_err());
            }
        }
        Command::CatFile(cat_file_args) => {
            let result = commands::cat_file(&cat_file_args.blob_name);
            if result.is_ok() {
                print!("{}", result.unwrap());
            } else {
                eprintln!("{}", result.unwrap_err());
            }
        }
    }
}
