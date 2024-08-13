use core::str;
use std::io::Read;

pub fn read_to_next_null_byte(reader: &mut impl Read) -> Result<String, String> {
    let mut bytes: Vec<u8> = Vec::new();
    loop {
        let byte = read_byte(reader)?;
        if byte == 0 {
            break;
        }
        bytes.push(byte);
    }
    return Ok(str::from_utf8(&bytes)
        .map_err(|err| format!("error converting bytes to utf8: {err}"))?
        .to_string());
}

pub fn read_byte(reader: &mut impl Read) -> Result<u8, String> {
    return Ok(read_n_bytes(1, reader)?[0]);
}

pub fn read_n_bytes(n: usize, reader: &mut impl Read) -> Result<Vec<u8>, String> {
    let mut buf = vec![0u8; n];
    reader
        .read_exact(&mut buf)
        .map_err(|err| format!("error while reading bytes: {err}"))?;
    return Ok(buf);
}

pub fn read_git_pack_line(reader: &mut impl Read) -> Result<Option<Vec<u8>>, String> {
    let length = usize::from_str_radix(
        str::from_utf8(&read_n_bytes(4, reader)?)
            .map_err(|err| format!("error converting pack line length to string: {err}"))?,
        16,
    )
    .map_err(|err| format!("error reading length of pack line: {err}"))?;
    return if length <= 4 {
        Ok(None)
    } else {
        Ok(Some(read_n_bytes(length - 4, reader)?))
    };
}
