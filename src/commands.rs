use core::str;
use std::{
    fs::{self, File},
    io::Read,
};

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
    let mut blob_contents: Vec<u8> = format!("blob {size}\0").bytes().into_iter().collect();
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
