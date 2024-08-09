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
            let result = commands::cat_file(&cat_file_args.object_name);
            if result.is_ok() {
                print!("{}", result.unwrap());
            } else {
                eprintln!("{}", result.unwrap_err());
            }
        }
        Command::HashObject(hash_object_args) => {
            let result = commands::hash_object(&hash_object_args.file_path, hash_object_args.write);
            if result.is_ok() {
                println!("{}", result.unwrap());
            } else {
                eprintln!("{}", result.unwrap_err());
            }
        }
        Command::LsTree(ls_tree_args) => {
            let result = commands::ls_tree(&ls_tree_args.object_name, ls_tree_args.name_only);
            if result.is_ok() {
                print!("{}", result.unwrap());
            } else {
                eprintln!("{}", result.unwrap_err());
            }
        }
        Command::WriteTree => {
            let result = commands::write_tree();
            if result.is_ok() {
                println!("{}", result.unwrap());
            } else {
                eprintln!("{}", result.unwrap_err());
            }
        }
    }
}
