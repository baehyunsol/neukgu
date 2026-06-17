use crate::{Error, hash_bytes};
use ragit_fs::{
    WriteMode,
    basename,
    create_dir,
    exists,
    join3,
    join4,
    read_dir,
    write_bytes,
};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::io::Cursor;
use std::sync::LazyLock;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct ImageId {
    // Hash value of bytes of an image *before* normalized.
    pub hash: u64,
    pub width: u32,
    pub height: u32,
}

impl ImageId {
    pub fn path(&self, working_dir: &str) -> Result<String, Error> {
        let images_dir = join3(
            working_dir,
            ".neukgu",
            "images",
        )?;
        Ok(join3(
            &images_dir,
            &format!("{:02x}", self.hash >> 56),
            &format!("{:016x}-{}x{}.png", self.hash, self.width, self.height),
        )?)
    }
}

static IMAGE_ID_RE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"([a-f0-9]{16})-(\d+)x(\d+)\.png").unwrap());

pub fn normalize_image(bytes: &[u8], working_dir: &str, size: u32) -> Result<ImageId, Error> {
    let hash = (hash_bytes(bytes) & 0xffff_ffff_ffff_ffff) as u64;
    let hash_str = format!("{hash:016x}");
    let prefix_dir = join4(
        working_dir,
        ".neukgu",
        "images",
        &format!("{:02x}", hash >> 56),
    )?;

    if !exists(&prefix_dir) {
        create_dir(&prefix_dir)?;
    }

    for e in read_dir(&prefix_dir, false)? {
        let basename = basename(&e)?;

        if basename.starts_with(&hash_str) {
            let Some(cap) = IMAGE_ID_RE.captures(&basename) else { break };
            let width = cap.get(2).unwrap().as_str().parse::<u32>().unwrap();
            let height = cap.get(3).unwrap().as_str().parse::<u32>().unwrap();
            return Ok(ImageId { hash, width, height });
        }
    }

    let mut image_buffer = image::load_from_memory(bytes)?;
    let image_id = ImageId {
        hash,
        width: image_buffer.width(),
        height: image_buffer.height(),
    };

    if image_id.width > size || image_id.height > size {
        image_buffer = image_buffer.resize(size, size, image::imageops::FilterType::Triangle);
    }

    let bytes = vec![];
    let mut writer = Cursor::new(bytes);
    image_buffer.write_to(&mut writer, image::ImageFormat::Png)?;
    let bytes = writer.into_inner();
    write_bytes(
        &image_id.path(working_dir)?,
        &bytes,
        WriteMode::Atomic,
    )?;

    Ok(image_id)
}
