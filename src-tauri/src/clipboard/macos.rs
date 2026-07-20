use super::common::{decode_image, image_content, text_hash};
use crate::model::ClipboardContent;
use arboard::{Clipboard, ImageData};
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
        if let Ok(image) = clipboard.get_image() {
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
        0
    }

    pub fn has_image_format(&self) -> bool {
        false
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
