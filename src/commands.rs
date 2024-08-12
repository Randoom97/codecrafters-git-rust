use core::str;
use std::{
    fs::{self, File},
    io::Read,
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::Local;

use crate::{
    git_object::{self, ObjectType},
    reader_utils,
};

pub fn init() -> Result<(), String> {
    fs::create_dir(".git").map_err(|err| format!("error creating .git directory: {err}"))?;
    fs::create_dir(".git/objects")
        .map_err(|err| format!("error creating objects directory: {err}"))?;
    fs::create_dir(".git/refs").map_err(|err| format!("error creating refs directory: {err}"))?;
    fs::write(".git/HEAD", "ref: refs/heads/main\n")
        .map_err(|err| format!("error writing HEAD file: {err}"))?;
    Ok(())
}

pub fn cat_file(object_name: &String) -> Result<String, String> {
    let mut reader = git_object::reader(object_name)?;
    let (object_type, size) =
        git_object::identify_header(&reader_utils::read_to_next_null_byte(&mut reader)?)?;

    match object_type {
        ObjectType::Commit | ObjectType::Blob => {
            let file_contents = reader_utils::read_n_bytes(size, &mut reader)?;
            return Ok(str::from_utf8(&file_contents)
                .map_err(|err| format!("error reading object file: {err}"))?
                .to_string());
        }
        ObjectType::Tree => return stringify_tree(&mut reader, size, false),
    }
}

pub fn hash_object(file_path: &String, write: bool) -> Result<String, String> {
    let mut file = File::open(&file_path).map_err(|err| format!("error opening file: {err}"))?;
    let mut file_contents: Vec<u8> = Vec::new();
    let size = file
        .read_to_end(&mut file_contents)
        .map_err(|err| format!("error reading file contents: {err}"))?;
    let mut blob_contents: Vec<u8> = format!("blob {size}\0").bytes().collect();
    blob_contents.append(&mut file_contents);

    let hash = if write {
        git_object::write_object(&blob_contents)?
    } else {
        git_object::hash_data(&blob_contents)
    };

    let hash_string = hex::encode(&hash);
    return Ok(hash_string);
}

pub fn ls_tree(object_name: &String, name_only: bool) -> Result<String, String> {
    let mut reader = git_object::reader(object_name)?;
    let (object_type, size) =
        git_object::identify_header(&reader_utils::read_to_next_null_byte(&mut reader)?)?;
    if object_type != ObjectType::Tree {
        return Err(format!("{object_name} is not a tree object"));
    }

    return stringify_tree(&mut reader, size, name_only);
}

pub fn write_tree() -> Result<String, String> {
    return Ok(hex::encode(git_object::write_tree_from_directory("./")?));
}

pub fn commit_tree(
    message: &String,
    tree_name: &String,
    parent_name: &Option<String>,
) -> Result<String, String> {
    if git_object::get_type(tree_name)? != ObjectType::Tree {
        return Err("provided hash isn't a tree".to_string());
    }
    if parent_name.is_some()
        && git_object::get_type(parent_name.as_ref().unwrap())? != ObjectType::Commit
    {
        return Err("provided parent hash isn't a commit".to_string());
    }

    let full_tree_name = git_object::full_hash(tree_name)?;
    let mut commit_byte_buffer: Vec<u8> = Vec::new();
    commit_byte_buffer.append(&mut format!("tree {full_tree_name}\n").bytes().collect());
    if parent_name.is_some() {
        let full_parent_hash = git_object::full_hash(parent_name.as_ref().unwrap())?;

        commit_byte_buffer.append(&mut format!("parent {}\n", full_parent_hash).bytes().collect());
    }
    let current_time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|err| format!("error getting timestamp: {err}"))?
        .as_secs();
    let timezone = Local::now().offset().to_string();
    commit_byte_buffer.append(
        &mut format!("author 123abc <123abc@example.com> {current_time} {timezone}\n")
            .bytes()
            .collect(),
    );
    commit_byte_buffer.append(
        &mut format!("committer 123abc <123abc@example.com> {current_time} {timezone}\n\n")
            .bytes()
            .collect(),
    );
    commit_byte_buffer.append(&mut format!("{message}\n").bytes().collect());

    let hash = git_object::write_commit(&mut commit_byte_buffer)?;

    return Ok(hex::encode(hash));
}

fn stringify_tree(reader: &mut impl Read, size: usize, name_only: bool) -> Result<String, String> {
    let tree_nodes = git_object::read_tree(reader, size)?;
    let mut result = String::new();
    for tree_node in tree_nodes {
        if name_only {
            result += format!("{}\n", tree_node.name).as_str();
        } else {
            let object_type = if tree_node.mode == 40000 {
                "tree"
            } else {
                "blob"
            };
            result += format!(
                "{:0>6} {} {}    {}\n",
                tree_node.mode, object_type, tree_node.hash, tree_node.name
            )
            .as_str();
        }
    }
    return Ok(result);
}
