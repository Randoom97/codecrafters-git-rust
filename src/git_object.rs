use flate2::bufread::ZlibDecoder;
use std::{
    fs::{self, File},
    io::{BufReader, Read},
    path::PathBuf,
};

pub enum ObjectType {
    Blob,
    Tree,
    Commit,
}

pub fn reader(blob_name: &String) -> Result<impl Read, String> {
    let path = path_for_blob(blob_name)?;
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

fn path_for_blob(blob_name: &String) -> Result<PathBuf, String> {
    if blob_name.len() < 2 {
        return Err("provided hash isn't long enough".to_string());
    }

    let directory = blob_name[..2].to_string();
    let filename = &blob_name[2..];
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
        return Err(format!("fatal: Not a valid object name {blob_name}"));
    }
    if paths.len() > 1 {
        return Err(format!(
            "fatal: Provided hash isn't unique enough {blob_name}"
        ));
    }

    return Ok(paths.pop().unwrap());
}
