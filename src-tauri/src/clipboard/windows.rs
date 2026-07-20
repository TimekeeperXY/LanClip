use super::common::{decode_image, image_content, text_hash};
use crate::model::ClipboardContent;
use arboard::{Clipboard, ImageData};
use image::ImageFormat;
use std::{borrow::Cow, sync::Mutex};

pub struct SystemClipboard {
    inner: Mutex<Clipboard>,
}

impl SystemClipboard {
    pub fn new() -> Result<Self, String> {
        Ok(Self {
            inner: Mutex::new(Clipboard::new().map_err(|e| e.to_string())?),
        })
    }

    pub fn read(&self) -> Option<(ClipboardContent, String)> {
        let mut clipboard = self.inner.lock().ok()?;

        let image = clipboard
            .get_image()
            .ok()
            .or_else(read_windows_legacy_image);
        if let Some(image) = image {
            if let Some(content) = image_content(image.width, image.height, &image.bytes) {
                return Some(content);
            }
        }

        if let Ok(text) = clipboard.get_text() {
            if !text.is_empty() {
                let hash = text_hash(&text);
                return Some((ClipboardContent::Text { text }, hash));
            }
        }
        None
    }

    pub fn sequence_number(&self) -> u32 {
        clipboard_win::seq_num().map_or(0, |number| number.get())
    }

    pub fn has_image_format(&self) -> bool {
        has_windows_image_format()
    }

    pub fn write(&self, content: &ClipboardContent) -> Result<String, String> {
        let mut clipboard = self.inner.lock().map_err(|_| "剪贴板暂时不可用")?;
        match content {
            ClipboardContent::Text { text } => {
                clipboard
                    .set_text(text.clone())
                    .map_err(|e| e.to_string())?;
                Ok(text_hash(text))
            }
            ClipboardContent::Image { .. } => {
                let (width, height, bytes, hash) = decode_image(content)?;
                clipboard
                    .set_image(ImageData {
                        width,
                        height,
                        bytes: Cow::Owned(bytes),
                    })
                    .map_err(|e| e.to_string())?;
                Ok(hash)
            }
        }
    }
}

fn has_windows_image_format() -> bool {
    use clipboard_win::{formats, raw};

    let png = clipboard_win::register_format("PNG")
        .is_some_and(|format| raw::is_format_avail(format.get()));
    png || raw::is_format_avail(formats::CF_DIBV5)
        || raw::is_format_avail(formats::CF_DIB)
        || raw::is_format_avail(formats::CF_BITMAP)
}

fn read_windows_legacy_image() -> Option<ImageData<'static>> {
    use clipboard_win::{formats, get_clipboard, raw, Format};

    let bitmap = formats::Bitmap
        .is_format_avail()
        .then(|| get_clipboard(formats::Bitmap).ok())
        .flatten();
    let bmp: Vec<u8> = bitmap.or_else(|| {
        if !raw::is_format_avail(formats::CF_DIB) {
            return None;
        }
        let dib: Vec<u8> = get_clipboard(formats::RawData(formats::CF_DIB)).ok()?;
        dib_to_bmp(&dib)
    })?;
    let rgba = image::load_from_memory_with_format(&bmp, ImageFormat::Bmp)
        .ok()?
        .to_rgba8();
    let (width, height) = rgba.dimensions();
    let bytes = rgba.into_raw();
    Some(ImageData {
        width: width as usize,
        height: height as usize,
        bytes: Cow::Owned(bytes),
    })
}

fn dib_to_bmp(dib: &[u8]) -> Option<Vec<u8>> {
    if dib.len() < 40 {
        return None;
    }
    let header_size = u32::from_le_bytes(dib[0..4].try_into().ok()?) as usize;
    if header_size < 40 || header_size > dib.len() {
        return None;
    }
    let bit_count = u16::from_le_bytes(dib[14..16].try_into().ok()?) as usize;
    let compression = u32::from_le_bytes(dib[16..20].try_into().ok()?);
    let colors_used = u32::from_le_bytes(dib[32..36].try_into().ok()?) as usize;
    let palette_entries = if colors_used > 0 {
        colors_used
    } else if bit_count <= 8 {
        1usize.checked_shl(bit_count as u32)?
    } else {
        0
    };
    let masks_size = if header_size == 40 {
        match compression {
            3 => 12,
            6 => 16,
            _ => 0,
        }
    } else {
        0
    };
    let pixel_offset = 14usize
        .checked_add(header_size)?
        .checked_add(masks_size)?
        .checked_add(palette_entries.checked_mul(4)?)?;
    if pixel_offset > 14 + dib.len() {
        return None;
    }

    let file_size = 14usize.checked_add(dib.len())?;
    let mut bmp = Vec::with_capacity(file_size);
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&(file_size as u32).to_le_bytes());
    bmp.extend_from_slice(&[0u8; 4]);
    bmp.extend_from_slice(&(pixel_offset as u32).to_le_bytes());
    bmp.extend_from_slice(dib);
    Some(bmp)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classic_cf_dib_is_converted_to_decodable_bmp() {
        let mut dib = Vec::new();
        dib.extend_from_slice(&40u32.to_le_bytes());
        dib.extend_from_slice(&1i32.to_le_bytes());
        dib.extend_from_slice(&1i32.to_le_bytes());
        dib.extend_from_slice(&1u16.to_le_bytes());
        dib.extend_from_slice(&32u16.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());
        dib.extend_from_slice(&4u32.to_le_bytes());
        dib.extend_from_slice(&0i32.to_le_bytes());
        dib.extend_from_slice(&0i32.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());
        dib.extend_from_slice(&0u32.to_le_bytes());
        dib.extend_from_slice(&[0, 0, 255, 255]);

        let bmp = dib_to_bmp(&dib).unwrap();
        let pixel = image::load_from_memory_with_format(&bmp, ImageFormat::Bmp)
            .unwrap()
            .to_rgba8();
        assert_eq!(pixel.dimensions(), (1, 1));
        assert_eq!(pixel.get_pixel(0, 0).0, [255, 0, 0, 255]);
    }
}
