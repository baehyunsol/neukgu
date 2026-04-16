use crate::Error;
use ragit_fs::{
    WriteMode,
    exists,
    join3,
    write_bytes,
};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

// This is a hash value of bytes of an image *before* normalized.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ImageId(pub u64);

impl ImageId {
    pub fn path(&self) -> Result<String, Error> {
        Ok(join3(
            ".neukgu",
            "images",
            &format!("{:016x}.png", self.0),
        )?)
    }
}

pub fn normalize_and_get_id(bytes: &[u8]) -> Result<ImageId, Error> {
    let hash = (hash_bytes(bytes) & 0xffff_ffff_ffff_ffff) as u64;
    let image_id = ImageId(hash);
    let image_path = image_id.path()?;

    if exists(&image_path) {
        return Ok(image_id);
    }

    let mut image_buffer = image::load_from_memory(bytes)?;

    if image_buffer.width() > 1024 || image_buffer.height() > 1024 {
        image_buffer = image_buffer.resize(1024, 1024, image::imageops::FilterType::Triangle);
    }

    let bytes = vec![];
    let mut writer = Cursor::new(bytes);
    image_buffer.write_to(&mut writer, image::ImageFormat::Png)?;
    let bytes = writer.into_inner();
    write_bytes(
        &image_path,
        &bytes,
        WriteMode::Atomic,
    )?;

    Ok(image_id)
}

pub fn hash_bytes(s: &[u8]) -> u128 {
    let mut r = 0;

    for (i, b) in s.iter().enumerate() {
        let c = (((r >> 24) & 0x00ff_ffff) << 24) | ((i & 0xfff) << 12) as u128 | *b as u128;
        let cc = c * c + c + 1;
        r += cc;
        r &= 0xffff_ffff_ffff_ffff_ffff_ffff;
    }

    r
}
