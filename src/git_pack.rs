use core::str;
use std::io::Read;

use flate2::read::ZlibDecoder;

use crate::{git_object, reader_utils};

pub fn unpack(reader: &mut impl Read) -> Result<(), String> {
    let mut pack_data: Vec<u8> = Vec::new();
    reader
        .read_to_end(&mut pack_data)
        .map_err(|err| format!("error reading pack data: {err}"))?;
    if str::from_utf8(&pack_data[..4]) != Ok("PACK") {
        return Err("not a valid pack".to_string());
    }

    let checksum = pack_data.split_off(pack_data.len() - 20);
    if checksum != git_object::hash_data(&pack_data) {
        return Err("pack data did not pass checksum".to_string());
    }

    let mut pack_buffer = &pack_data[..];
    reader_utils::read_n_bytes(8, &mut pack_buffer)?; // skip 'PACK' and version

    let object_count = u32::from_be_bytes(
        reader_utils::read_n_bytes(4, &mut pack_buffer)?
            .try_into()
            .unwrap(),
    );

    for _ in 0..object_count {
        let (o_type, size) = read_type_and_size(&mut pack_buffer)?;
        match o_type {
            ObjectType::Commit => {
                git_object::write_commit(&mut zlib_read(size, &mut pack_buffer)?)?;
            }
            ObjectType::Tree => {
                git_object::write_tree(&mut zlib_read(size, &mut pack_buffer)?)?;
            }
            ObjectType::Blob => {
                git_object::write_blob(&mut zlib_read(size, &mut pack_buffer)?)?;
            }
            ObjectType::Tag => {
                // unsupported
                zlib_read(size, &mut pack_buffer)?;
            }
            ObjectType::OfsDelta => {
                return Err("offset deltas not currently supported".to_string());
            }
            ObjectType::RefDelta => {
                let reference_hash = reader_utils::read_n_bytes(20, &mut pack_buffer)?;
                let data = zlib_read(size, &mut pack_buffer)?;
                let target_data = apply_delta(&hex::encode(reference_hash), &data)?;
                git_object::write_object(&target_data)?;
            }
        }
    }

    return Ok(());
}

fn apply_delta(reference_hash: &String, delta: &Vec<u8>) -> Result<Vec<u8>, String> {
    let mut delta_buffer = &delta[..];
    let source_length = read_size(&mut delta_buffer)?;
    let target_length = read_size(&mut delta_buffer)?;

    let object_type = git_object::get_type(reference_hash)?;
    let mut source_reader = git_object::reader(reference_hash)?;
    reader_utils::read_to_next_null_byte(&mut source_reader)?;
    let mut source_data = Vec::new();
    source_reader
        .read_to_end(&mut source_data)
        .map_err(|err| format!("error reading reference object: {err}"))?;
    if source_data.len() != source_length {
        return Err("source object wasn't the correct length for de deltifying".to_string());
    }

    let mut target_data: Vec<u8> = Vec::new();
    while delta_buffer.len() > 0 {
        let command = reader_utils::read_byte(&mut delta_buffer)?;
        if command & 0b10000000 == 0 {
            // insert
            target_data.append(&mut reader_utils::read_n_bytes(
                (command & 0b1111111).into(),
                &mut delta_buffer,
            )?);
            continue;
        }
        // copy
        let mut offset: usize = 0;
        for i in 0..4 {
            if command & (0b1 << i) != 0 {
                let offset_byte: usize = reader_utils::read_byte(&mut delta_buffer)?.into();
                offset |= offset_byte << (8 * i);
            }
        }
        let mut size: usize = 0;
        for i in 0..3 {
            if command & (0b10000 << i) != 0 {
                let size_byte: usize = reader_utils::read_byte(&mut delta_buffer)?.into();
                size |= size_byte << (8 * i);
            }
        }

        target_data.append(&mut source_data[offset..(offset + size)].to_vec());
    }

    if target_data.len() != target_length {
        return Err("target object wasn't the correct length for de deltifying".to_string());
    }

    let mut object_bytes: Vec<u8> = format!("{} {}\0", object_type, target_data.len())
        .bytes()
        .collect();
    object_bytes.append(&mut target_data);
    return Ok(object_bytes);
}

#[derive(Debug)]
enum ObjectType {
    Commit,
    Tree,
    Blob,
    Tag,
    OfsDelta,
    RefDelta,
}

fn read_type_and_size(reader: &mut impl Read) -> Result<(ObjectType, usize), String> {
    let first_byte = reader_utils::read_byte(reader)?;
    let o_type = match (first_byte & 0b01110000) >> 4 {
        0b001 => ObjectType::Commit,
        0b010 => ObjectType::Tree,
        0b011 => ObjectType::Blob,
        0b100 => ObjectType::Tag,
        0b110 => ObjectType::OfsDelta,
        0b111 => ObjectType::RefDelta,
        pack_object_type => {
            return Err(format!(
                "unknown pack object type: {:0<3b}",
                pack_object_type
            ))
        }
    };
    let mut size: usize = (first_byte & 0b1111).into();

    if first_byte & 0b10000000 == 0 {
        return Ok((o_type, size));
    }

    let mut bytes_read = 1;
    loop {
        let b: usize = reader_utils::read_byte(reader)?.into();
        bytes_read += 1;
        size |= (b & 0b1111111) << ((bytes_read - 2) * 7 + 4);
        if b & 0b10000000 == 0 {
            break;
        }
    }
    return Ok((o_type, size));
}

fn read_size(reader: &mut impl Read) -> Result<usize, String> {
    let mut size = 0;
    let mut bytes_read = 0;
    loop {
        let b: usize = reader_utils::read_byte(reader)?.into();
        bytes_read += 1;
        size |= (b & 0b1111111) << ((bytes_read - 1) * 7);
        if b & 0b10000000 == 0 {
            break;
        }
    }
    return Ok(size);
}

fn zlib_read(size: usize, reader: &mut impl Read) -> Result<Vec<u8>, String> {
    let mut zlib_reader = ZlibDecoder::new_with_buf(reader, vec![0; 1]); // giving this a buffer size of 1 so it doesn't overconsume from reader
    let mut data = Vec::new();
    let size_read = zlib_reader
        .read_to_end(&mut data)
        .map_err(|err| format!("error uncompressing pack object: {err}"))?;
    if size_read != size {
        return Err(format!(
            "expected object length of {size}, got length {size_read}"
        ));
    }
    return Ok(data);
}
