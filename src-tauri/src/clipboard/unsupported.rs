use crate::model::ClipboardContent;

pub struct SystemClipboard;

impl SystemClipboard {
    pub fn new() -> Result<Self, String> {
        Err("LanClip 当前仅支持 Windows 和 macOS 剪贴板".into())
    }

    pub fn read(&self) -> Option<(ClipboardContent, String)> {
        None
    }

    pub fn sequence_number(&self) -> u32 {
        0
    }

    pub fn has_image_format(&self) -> bool {
        false
    }

    pub fn write(&self, _content: &ClipboardContent) -> Result<String, String> {
        Err("当前平台暂不支持剪贴板写入".into())
    }
}
