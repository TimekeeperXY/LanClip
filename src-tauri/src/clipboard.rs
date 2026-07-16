use crate::{
    crypto::content_hash,
    model::{ClipboardContent, MAX_IMAGE_BYTES},
};
use arboard::{Clipboard, ImageData};
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{DynamicImage, ImageFormat, RgbaImage};
use std::{borrow::Cow, io::Cursor, sync::Mutex};

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
            if image.bytes.len() <= MAX_IMAGE_BYTES {
                let hash = hash_image(image.width, image.height, &image.bytes);
                return Some((
                    ClipboardContent::Image {
                        width: image.width,
                        height: image.height,
                        rgba_base64: STANDARD.encode(image.bytes.as_ref()),
                    },
                    hash,
                ));
            }
        }

        if let Ok(text) = clipboard.get_text() {
            if !text.is_empty() {
                let hash = content_hash(text.as_bytes());
                return Some((ClipboardContent::Text { text }, hash));
            }
        }
        None
    }

    pub fn sequence_number(&self) -> u32 {
        clipboard_sequence_number()
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
                Ok(content_hash(text.as_bytes()))
            }
            ClipboardContent::Image {
                width,
                height,
                rgba_base64,
            } => {
                let bytes = STANDARD.decode(rgba_base64).map_err(|e| e.to_string())?;
                let expected = width.saturating_mul(*height).saturating_mul(4);
                if bytes.len() != expected || bytes.len() > MAX_IMAGE_BYTES {
                    return Err("图片数据无效或超过 20 MB".into());
                }
                let hash = hash_image(*width, *height, &bytes);
                clipboard
                    .set_image(ImageData {
                        width: *width,
                        height: *height,
                        bytes: Cow::Owned(bytes),
                    })
                    .map_err(|e| e.to_string())?;
                Ok(hash)
            }
        }
    }
}

pub fn thumbnail_data_url(content: &ClipboardContent) -> Option<String> {
    let ClipboardContent::Image {
        width,
        height,
        rgba_base64,
    } = content
    else {
        return None;
    };
    let bytes = STANDARD.decode(rgba_base64).ok()?;
    if bytes.len() != width.saturating_mul(*height).saturating_mul(4) {
        return None;
    }
    let rgba = RgbaImage::from_raw(*width as u32, *height as u32, bytes)?;
    let thumbnail = DynamicImage::ImageRgba8(rgba).thumbnail(192, 132);
    let mut output = Cursor::new(Vec::new());
    thumbnail.write_to(&mut output, ImageFormat::Png).ok()?;
    Some(format!(
        "data:image/png;base64,{}",
        STANDARD.encode(output.into_inner())
    ))
}

#[cfg(windows)]
fn clipboard_sequence_number() -> u32 {
    clipboard_win::seq_num().map_or(0, |number| number.get())
}

#[cfg(windows)]
fn has_windows_image_format() -> bool {
    use clipboard_win::{formats, raw};

    let png = clipboard_win::register_format("PNG")
        .is_some_and(|format| raw::is_format_avail(format.get()));
    png || raw::is_format_avail(formats::CF_DIBV5)
        || raw::is_format_avail(formats::CF_DIB)
        || raw::is_format_avail(formats::CF_BITMAP)
}

#[cfg(not(windows))]
fn has_windows_image_format() -> bool {
    false
}

#[cfg(not(windows))]
fn clipboard_sequence_number() -> u32 {
    0
}

#[cfg(windows)]
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
    if bytes.len() > MAX_IMAGE_BYTES {
        return None;
    }
    Some(ImageData {
        width: width as usize,
        height: height as usize,
        bytes: Cow::Owned(bytes),
    })
}

#[cfg(not(windows))]
fn read_windows_legacy_image() -> Option<ImageData<'static>> {
    None
}

#[cfg(windows)]
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

fn hash_image(width: usize, height: usize, bytes: &[u8]) -> String {
    let mut input = Vec::with_capacity(bytes.len() + 16);
    input.extend_from_slice(&width.to_le_bytes());
    input.extend_from_slice(&height.to_le_bytes());
    input.extend_from_slice(bytes);
    content_hash(&input)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn image_thumbnail_is_embedded_as_png_data_url() {
        let content = ClipboardContent::Image {
            width: 2,
            height: 2,
            rgba_base64: STANDARD.encode([
                255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 255, 255,
            ]),
        };
        let thumbnail = thumbnail_data_url(&content).unwrap();
        assert!(thumbnail.starts_with("data:image/png;base64,"));
    }

    #[test]
    fn text_has_no_image_thumbnail() {
        assert!(thumbnail_data_url(&ClipboardContent::Text { text: "hi".into() }).is_none());
    }

    #[cfg(windows)]
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
