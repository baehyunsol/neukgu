use crate::{Error, hash_bytes};
use ragit_fs::{
    WriteMode,
    exists,
    join4,
    write_bytes,
};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

// This is a hash value of bytes of an image *before* normalized.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ImageId(pub u64);

impl ImageId {
    pub fn path(&self, working_dir: &str) -> Result<String, Error> {
        Ok(join4(
            working_dir,
            ".neukgu",
            "images",
            &format!("{:016x}.png", self.0),
        )?)
    }
}

pub fn normalize_and_get_id(bytes: &[u8], working_dir: &str) -> Result<ImageId, Error> {
    let hash = (hash_bytes(bytes) & 0xffff_ffff_ffff_ffff) as u64;
    let image_id = ImageId(hash);
    let image_path = image_id.path(working_dir)?;

    if exists(&image_path) {
        return Ok(image_id);
    }

    let mut image_buffer = image::load_from_memory(bytes)?;

    if image_buffer.width() > 1200 || image_buffer.height() > 1200 {
        image_buffer = image_buffer.resize(1200, 1200, image::imageops::FilterType::Triangle);
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
