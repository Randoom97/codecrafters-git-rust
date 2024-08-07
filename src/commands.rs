use core::str;
use std::fs;

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

pub fn cat_file(blob_name: &String) -> Result<String, String> {
    let mut reader = git_object::reader(blob_name)?;
    let (object_type, size) =
        git_object::identify_header(&reader_utils::read_to_next_null_byte(&mut reader)?)?;

    match object_type {
        ObjectType::Commit | ObjectType::Blob => {
            let file_contents = reader_utils::read_n_bytes(size, &mut reader)?;
            return Ok(str::from_utf8(&file_contents)
                .map_err(|err| format!("error reading object file: {err}"))?
                .to_string());
        }
        ObjectType::Tree => return Err("cat-file not implemented for trees".to_string()),
    }
}
