use crate::{
    crypto::content_hash,
    model::{ClipboardContent, MAX_IMAGE_BYTES},
};
use base64::{engine::general_purpose::STANDARD, Engine};
use image::{DynamicImage, ImageFormat, RgbaImage};
use std::io::Cursor;

pub fn text_hash(text: &str) -> String {
    content_hash(text.as_bytes())
}

pub fn hash_image(width: usize, height: usize, bytes: &[u8]) -> String {
    let mut input = Vec::with_capacity(bytes.len() + 16);
    input.extend_from_slice(&width.to_le_bytes());
    input.extend_from_slice(&height.to_le_bytes());
    input.extend_from_slice(bytes);
    content_hash(&input)
}

pub fn image_content(
    width: usize,
    height: usize,
    bytes: &[u8],
) -> Option<(ClipboardContent, String)> {
    if bytes.len() > MAX_IMAGE_BYTES {
        return None;
    }
    let expected = width.checked_mul(height)?.checked_mul(4)?;
    if bytes.len() != expected {
        return None;
    }
    let hash = hash_image(width, height, bytes);
    Some((
        ClipboardContent::Image {
            width,
            height,
            rgba_base64: STANDARD.encode(bytes),
        },
        hash,
    ))
}

pub fn decode_image(content: &ClipboardContent) -> Result<(usize, usize, Vec<u8>, String), String> {
    let ClipboardContent::Image {
        width,
        height,
        rgba_base64,
    } = content
    else {
        return Err("剪贴板内容不是图片".into());
    };
    let bytes = STANDARD.decode(rgba_base64).map_err(|e| e.to_string())?;
    let expected = width.saturating_mul(*height).saturating_mul(4);
    if bytes.len() != expected || bytes.len() > MAX_IMAGE_BYTES {
        return Err("图片数据无效或超过 20 MB".into());
    }
    let hash = hash_image(*width, *height, &bytes);
    Ok((*width, *height, bytes, hash))
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

    #[test]
    fn image_content_rejects_invalid_rgba_lengths() {
        assert!(image_content(2, 2, &[255, 0, 0, 255]).is_none());
    }
}
