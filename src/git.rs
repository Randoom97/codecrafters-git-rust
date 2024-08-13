use core::str;
use std::{fs, io::Read};

use crate::{
    git_object::{self, ObjectType},
    reader_utils,
};

pub fn make_branch(reference: &String, hash: &String) -> Result<(), String> {
    let object_type = git_object::get_type(hash)?;
    if object_type != ObjectType::Commit {
        return Err(format!(
            "{hash} isn't a commit and so can't be made a branch"
        ));
    }

    fs::create_dir_all(".git/refs/heads")
        .map_err(|err| format!("error creating heads directory: {err}"))?;

    fs::write(format!(".git/refs/heads/{reference}"), format!("{hash}\n"))
        .map_err(|err| format!("error writing to heads/{reference}: {err}"))?;

    return Ok(());
}

pub fn checkout(reference: &String) -> Result<(), String> {
    let mut hash = fs::read_to_string(format!(".git/refs/heads/{reference}"))
        .map_err(|err| format!("error reading refs/heads/{reference}: {err}"))?;
    hash.pop(); // remove trailing "\n"

    fs::write(
        format!(".git/HEAD"),
        format!("ref: refs/heads/{reference}\n"),
    )
    .map_err(|err| format!("error writing to refs/HEAD: {err}"))?;

    let mut commit_reader = git_object::reader(&hash)?;
    reader_utils::read_to_next_null_byte(&mut commit_reader)?;
    reader_utils::read_n_bytes(5, &mut commit_reader)?; // 'tree '
    let hash_bytes = reader_utils::read_n_bytes(20, &mut commit_reader)?;
    let tree_hash = str::from_utf8(&hash_bytes)
        .map_err(|err| format!("error parsing tree hash to string: {err}"))?;
    return construct_tree(&"./".to_string(), &tree_hash.to_string());
}

fn construct_tree(path: &String, hash: &String) -> Result<(), String> {
    let mut tree_reader = git_object::reader(hash)?;

    let (_, length) =
        git_object::identify_header(&reader_utils::read_to_next_null_byte(&mut tree_reader)?)?;
    let tree_nodes = git_object::read_tree(&mut tree_reader, length)?;

    for tree_node in tree_nodes {
        if tree_node.mode == 40000 {
            fs::create_dir_all(format!("{}{}", path, tree_node.name))
                .map_err(|err| format!("error creating directory for {}: {err}", tree_node.name))?;
            construct_tree(&format!("{}{}/", path, tree_node.name), &tree_node.hash)?;
        } else {
            construct_blob(path, &tree_node.name, &tree_node.hash)?;
        }
    }
    return Ok(());
}

fn construct_blob(path: &String, name: &String, hash: &String) -> Result<(), String> {
    let mut blob_reader = git_object::reader(hash)?;
    reader_utils::read_to_next_null_byte(&mut blob_reader)?;
    let mut blob_data = Vec::new();
    blob_reader
        .read_to_end(&mut blob_data)
        .map_err(|err| format!("error reading blob object: {err}"))?;
    return fs::write(format!("{path}{name}"), blob_data)
        .map_err(|err| format!("error writing file: {err}"));
}
