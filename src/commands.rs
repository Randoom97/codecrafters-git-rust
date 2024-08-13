use core::str;
use std::{
    env::set_current_dir,
    fs::{self, File},
    io::Read,
    time::{SystemTime, UNIX_EPOCH},
};

use chrono::Local;
use reqwest::{blocking::Client, header::CONTENT_TYPE, Method, StatusCode};

use crate::{
    git,
    git_object::{self, ObjectType},
    git_pack, reader_utils,
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

pub fn clone(remote: &String, directory: &String) -> Result<String, String> {
    let mut remote_url = remote.clone();
    if remote_url.ends_with('/') {
        remote_url.pop();
    }

    let client = Client::new();
    let mut discovery_response = client
        .request(
            Method::GET,
            format!("{remote_url}/info/refs?service=git-upload-pack"),
        )
        .send()
        .map_err(|err| format!("error sending discovery request: {err}"))?;
    if discovery_response.status() != StatusCode::OK {
        return Err(format!("discovery status: {}", discovery_response.status()));
    }

    // junk lines before ref data
    let mut data = reader_utils::read_git_pack_line(&mut discovery_response)?;
    if data.is_some() {
        reader_utils::read_git_pack_line(&mut discovery_response)?;
    }

    // first ref has trailing capabilities
    data = reader_utils::read_git_pack_line(&mut discovery_response)?;
    if data.is_none() {
        return Err("no data found where refs should have started".to_string());
    }

    let ref_parts = str::from_utf8(data.as_ref().unwrap())
        .map_err(|err| format!("error converting pack data to string: {err}"))?
        .split("\x00")
        .collect::<Vec<&str>>()[0]
        .split(" ")
        .collect::<Vec<&str>>();
    if ref_parts.get(1) != Some(&"HEAD") {
        return Err("no HEAD ref advertized".to_string());
    }
    let head_hash = ref_parts[0];

    let mut head_ref: Option<String> = None;
    loop {
        let data = reader_utils::read_git_pack_line(&mut discovery_response)?;
        if data.is_none() {
            break;
        }

        let ref_parts: Vec<&str> = str::from_utf8(data.as_ref().unwrap())
            .map_err(|err| format!("error converting pack data to string: {err}"))?
            .split(" ")
            .collect();
        if ref_parts.get(0) != Some(&head_hash) {
            continue;
        }
        head_ref = ref_parts.get(1).copied().map(|r| {
            let mut s = r.to_string();
            if s.ends_with("\n") {
                s.pop();
            }
            return s;
        });
    }
    if head_ref.is_none() {
        return Err("a ref that matches HEAD could not be found".to_string());
    }
    let ref_name = head_ref
        .as_ref()
        .unwrap()
        .rsplit_once('/')
        .map(|(_, right)| right.to_string())
        .unwrap_or(head_ref.unwrap());

    let pack_body: Vec<u8> = format!("0032want {}\n00000009done\n", head_hash)
        .bytes()
        .collect();
    let mut pack_response = client
        .request(
            Method::POST,
            format!("{remote_url}/git-upload-pack?service=git-upload-pack"),
        )
        .header(CONTENT_TYPE, "application/x-git-upload-pack-request")
        .body(pack_body)
        .send()
        .map_err(|err| format!("error sending pack data request: {err}"))?;
    if pack_response.status() != StatusCode::OK {
        return Err(format!("pack status: {}", pack_response.status()));
    }

    fs::create_dir_all(directory).map_err(|err| format!("error creating directory: {err}"))?;
    set_current_dir(directory).map_err(|err| format!("error changing directory: {err}"))?;
    init()?;

    reader_utils::read_git_pack_line(&mut pack_response)?; // NAK
    git_pack::unpack(&mut pack_response)?;
    git::make_branch(&ref_name, &head_hash.to_string())?;
    git::checkout(&ref_name)?;

    return Ok(format!("cloned remote {remote_url} to {directory}"));
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
