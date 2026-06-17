use crate::{Error, ImageId, check_interruption, hash_bytes, load_json, normalize_image};
use hayro::{RenderSettings, render};
use hayro::hayro_syntax::Pdf;
use hayro::vello_cpu::color::palette::css::WHITE;
use ragit_fs::{
    WriteMode,
    exists,
    join4,
    write_string,
};
use serde::{Deserialize, Serialize};
use std::io::Cursor;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
pub struct PdfId(pub u64);

impl PdfId {
    pub fn pages_path(&self, working_dir: &str) -> Result<String, Error> {
        Ok(join4(
            working_dir,
            ".neukgu",
            "pdfs",
            &format!("{:016x}.json", self.0),
        )?)
    }

    pub fn get_pages(&self, working_dir: &str) -> Result<Vec<ImageId>, Error> {
        Ok(load_json(&self.pages_path(working_dir)?)?)
    }

    // pub fn count_pages(&self) -> u64 {
    //     self.0 & 0xffff
    // }
}

pub fn render_pdf(bytes: &[u8], working_dir: &str) -> Result<PdfId, Error> {
    let pdf = Pdf::new(bytes.to_vec())?;
    let id = ((hash_bytes(bytes) & 0xffff_ffff_ffff) << 16) as u64 | pdf.pages().len() as u64;
    let id = PdfId(id);

    if exists(&id.pages_path(working_dir)?) {
        return Ok(id);
    }

    let mut pages = Vec::with_capacity(pdf.len());

    for page in pdf.pages().iter() {
        let pixmap = render(
            page,
            &Default::default(),
            &Default::default(),
            &RenderSettings {
                // hayro's default resolution is too small...
                x_scale: 2.0,
                y_scale: 2.0,

                bg_color: WHITE,
                ..Default::default()
            },
        );
        let png_bytes = pixmap.into_png()?;
        let image_id = normalize_image(&png_bytes, working_dir, 1200)?;
        pages.push(image_id);

        if check_interruption(working_dir)? {
            return Err(Error::UserInterrupt);
        }
    }

    write_string(
        &id.pages_path(working_dir)?,
        &serde_json::to_string_pretty(&pages)?,
        WriteMode::Atomic,
    )?;
    Ok(id)
}

pub fn render_first_few_pages_of_pdf(bytes: &[u8], few: usize, resolution: u32) -> Result<Option<(Vec<Vec<u8>>, usize)>, Error> {
    match Pdf::new(bytes.to_vec()) {
        // hayro sometimes treats non-pdf files as a zero-page pdf file
        Ok(pdf) if pdf.pages().len() > 0 => {
            let total_pages = pdf.pages().len();
            let mut pages = vec![];

            for page in pdf.pages().iter().take(few) {
                let pixmap = render(
                    page,
                    &Default::default(),
                    &Default::default(),
                    &RenderSettings {
                        // hayro's default resolution is too small...
                        x_scale: 2.0,
                        y_scale: 2.0,

                        bg_color: WHITE,
                        ..Default::default()
                    },
                );
                let png_bytes = pixmap.into_png()?;
                let mut image_buffer = image::load_from_memory(&png_bytes)?;

                if image_buffer.width() > resolution || image_buffer.height() > resolution {
                    image_buffer = image_buffer.resize(resolution, resolution, image::imageops::FilterType::Triangle);
                }

                let bytes = vec![];
                let mut writer = Cursor::new(bytes);
                image_buffer.write_to(&mut writer, image::ImageFormat::Png)?;
                let bytes = writer.into_inner();
                pages.push(bytes);
            }

            Ok(Some((pages, total_pages)))
        },
        _ => Ok(None),
    }
}
