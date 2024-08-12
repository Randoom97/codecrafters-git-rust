use flate2::{bufread::ZlibDecoder, write::ZlibEncoder, Compression};
use sha1::{Digest, Sha1};
use std::{
    fs::{self, File},
    io::{BufReader, Read, Write},
    path::{Path, PathBuf},
};

use crate::reader_utils;

#[derive(PartialEq)]
pub enum ObjectType {
    Blob,
    Tree,
    Commit,
}

pub fn write_object(data: &Vec<u8>) -> Result<Vec<u8>, String> {
    let hash = hash_data(data);
    let hash_string = hex::encode(&hash);
    let directory = &hash_string[..2];
    let file_name = &hash_string[2..];

    fs::create_dir_all(format!(".git/objects/{directory}"))
        .map_err(|err| format!("error creating directory for git object: {err}"))?;
    let file = File::create(format!(".git/objects/{directory}/{file_name}"))
        .map_err(|err| format!("error creating file for git object: {err}"))?;

    let mut encoder = ZlibEncoder::new(file, Compression::default());
    encoder
        .write_all(data)
        .map_err(|err| format!("error compressing git object: {err}"))?;

    return Ok(hash);
}

pub fn write_blob_from_file<P: AsRef<Path>>(file_path: P) -> Result<Vec<u8>, String> {
    let mut file_bytes = fs::read(file_path).map_err(|err| format!("error reading file {err}"))?;
    return write_blob(&mut file_bytes);
}

pub fn write_blob(data: &mut Vec<u8>) -> Result<Vec<u8>, String> {
    let mut blob_bytes: Vec<u8> = format!("blob {}\0", data.len()).bytes().collect();
    blob_bytes.append(data);
    return write_object(&blob_bytes);
}

pub fn write_tree_from_directory<P: AsRef<Path>>(directory_path: P) -> Result<Vec<u8>, String> {
    let mut paths: Vec<PathBuf> = fs::read_dir(directory_path)
        .map_err(|err| format!("error reading directory: {err}"))?
        .into_iter()
        .filter(|r| r.is_ok())
        .map(|r| r.unwrap().path())
        .collect();
    paths.sort();

    let mut tree_byte_buffer: Vec<u8> = Vec::new();
    for path in paths {
        let name = path
            .file_name()
            .map(|os_name| os_name.to_str())
            .unwrap_or(None);
        if name.is_none() {
            return Err("error getting name for dir entry".to_string());
        }

        if name.unwrap() == ".git" {
            continue;
        }

        let (mut entry_hash, mode) = if path.is_dir() {
            (write_tree_from_directory(&path)?, 40000)
        } else {
            (write_blob_from_file(&path)?, 100644)
        };

        tree_byte_buffer.append(&mut format!("{} {}\0", mode, name.unwrap()).bytes().collect());
        tree_byte_buffer.append(&mut entry_hash);
    }

    return write_tree(&mut tree_byte_buffer);
}

pub fn write_tree(data: &mut Vec<u8>) -> Result<Vec<u8>, String> {
    let mut tree_bytes: Vec<u8> = format!("tree {}\0", data.len()).bytes().collect();
    tree_bytes.append(data);
    return write_object(&tree_bytes);
}

pub fn write_commit(data: &mut Vec<u8>) -> Result<Vec<u8>, String> {
    let mut commit_bytes: Vec<u8> = format!("commit {}\0", data.len()).bytes().collect();
    commit_bytes.append(data);
    return write_object(&commit_bytes);
}

pub fn reader(object_name: &String) -> Result<impl Read, String> {
    let path = path_for_object(object_name)?;
    let f = File::open(path).map_err(|err| format!("error opening file: {err}"))?;
    let reader = BufReader::new(f);
    let decoder = ZlibDecoder::new(reader);
    return Ok(decoder);
}

pub fn identify_header(header: &String) -> Result<(ObjectType, usize), String> {
    let parts: Vec<&str> = header.split(' ').collect();
    if parts.len() != 2 {
        return Err("git object header didn't have the correct amount of parts".to_string());
    }

    let object_type = match parts[0] {
        "blob" => ObjectType::Blob,
        "tree" => ObjectType::Tree,
        "commit" => ObjectType::Commit,
        o_type => return Err(format!("unknown object type: {o_type}")),
    };

    let size = str::parse::<usize>(parts[1])
        .map_err(|err| format!("error parsing git object size: {err}"))?;

    return Ok((object_type, size));
}

fn path_for_object(object_name: &String) -> Result<PathBuf, String> {
    if object_name.len() < 2 {
        return Err("provided hash isn't long enough".to_string());
    }

    let directory = object_name[..2].to_string();
    let filename = &object_name[2..];
    let mut paths: Vec<PathBuf> = fs::read_dir(format!(".git/objects/{directory}/"))
        .map_err(|err| format!("error reading objects directory: {err}"))?
        .into_iter()
        .filter(|r| r.is_ok())
        .map(|r| r.unwrap().path())
        .filter(|r| {
            r.file_name()
                .is_some_and(|name| name.to_str().is_some_and(|str| str.starts_with(filename)))
        })
        .collect();

    if paths.len() < 1 {
        return Err(format!("fatal: Not a valid object name {object_name}"));
    }
    if paths.len() > 1 {
        return Err(format!(
            "fatal: Provided hash isn't unique enough {object_name}"
        ));
    }

    return Ok(paths.pop().unwrap());
}

pub fn hash_data(data: &Vec<u8>) -> Vec<u8> {
    let mut hasher = Sha1::new();
    hasher.update(data);
    return hasher.finalize().into_iter().collect();
}

pub struct TreeNode {
    pub mode: u64,
    pub name: String,
    pub hash: String,
}

pub fn read_tree(reader: &mut impl Read, mut size: usize) -> Result<Vec<TreeNode>, String> {
    let mut result: Vec<TreeNode> = Vec::new();
    while size > 0 {
        let info = reader_utils::read_to_next_null_byte(reader)?;
        let parts: Vec<&str> = info.split(' ').into_iter().collect();
        if parts.len() != 2 {
            return Err("tree info had the incorrect amount of parts".to_string());
        }

        let mode = str::parse::<u64>(parts[0])
            .map_err(|err| format!("error parsing tree node mode: {err}"))?;

        result.push(TreeNode {
            mode,
            name: parts[1].to_string(),
            hash: hex::encode(reader_utils::read_n_bytes(20, reader)?),
        });

        size -= info.len() + 21;
    }
    return Ok(result);
}

pub fn get_type(hash: &String) -> Result<ObjectType, String> {
    let mut reader = reader(hash)?;
    let (object_type, _) = identify_header(&reader_utils::read_to_next_null_byte(&mut reader)?)?;
    return Ok(object_type);
}

pub fn full_hash(partial_hash: &String) -> Result<String, String> {
    let path = path_for_object(partial_hash)?;
    let path_str = path.to_str();
    if path_str.is_none() {
        return Err("error converting path to string".to_string());
    }
    let parts: Vec<&str> = path_str.unwrap().split("/").collect();
    return Ok(parts[parts.len() - 2..].join(""));
}
