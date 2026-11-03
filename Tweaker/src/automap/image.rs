use crate::{Error, Result};
use std::ops::Range;

pub struct Image {
    pub base: usize,
    pub bytes: Vec<u8>,
    pub sections: Vec<Section>,
    readable: Vec<Range<usize>>,
}
pub struct Section {
    pub range: Range<usize>,
    pub executable: bool,
    pub writable: bool,
}

impl Image {
    pub fn capture(base: usize, size: usize) -> Result<Self> {
        if !(0x1000..=200 * 1024 * 1024).contains(&size) {
            return Err(Error::Jvm("automap: invalid image size".into()));
        }
        let mut bytes = vec![0; size];
        let mut readable: Vec<Range<usize>> = Vec::new();
        for start in (0..size).step_by(0x1000) {
            let len = (size - start).min(0x1000);
            if let Some(page) = crate::slinky::mem::read_memory(base + start, len) {
                bytes[start..start + len].copy_from_slice(&page);
                if let Some(last) = readable.last_mut().filter(|last| last.end == start) {
                    last.end += len;
                } else {
                    readable.push(start..start + len);
                }
            }
        }
        let mut image = Self {
            base,
            bytes,
            sections: Vec::new(),
            readable,
        };
        image.parse_sections()?;
        Ok(image)
    }
    fn parse_sections(&mut self) -> Result<()> {
        let invalid = || Error::Jvm("automap: invalid or unreadable x64 PE headers".into());
        if self.read(0, 2) != Some(&b"MZ"[..]) {
            return Err(invalid());
        }
        let pe = self.u32(0x3c).ok_or_else(invalid)? as usize;
        if pe > 0x1000 || self.read(pe, 4) != Some(&b"PE\0\0"[..]) {
            return Err(invalid());
        }
        if self.u16(pe + 4) != Some(0x8664) || self.u16(pe + 24) != Some(0x20b) {
            return Err(invalid());
        }
        let count = self.u16(pe + 6).ok_or_else(invalid)? as usize;
        let optional = self.u16(pe + 20).ok_or_else(invalid)? as usize;
        let size = self.u32(pe + 0x50).ok_or_else(invalid)? as usize;
        if count == 0 || count > 96 || size != self.bytes.len() || optional < 0x70 {
            return Err(invalid());
        }
        for i in 0..count {
            let at = pe + 24 + optional + i * 40;
            let start = self.u32(at + 12).ok_or_else(invalid)? as usize;
            let len = self.u32(at + 8).ok_or_else(invalid)? as usize;
            let flags = self.u32(at + 36).ok_or_else(invalid)?;
            let end = start
                .checked_add(len)
                .filter(|&end| end <= size)
                .ok_or_else(invalid)?;
            self.sections.push(Section {
                range: start..end,
                executable: flags & 0x20000000 != 0,
                writable: flags & 0x80000000 != 0,
            });
        }
        Ok(())
    }
    pub fn read(&self, at: usize, len: usize) -> Option<&[u8]> {
        let end = at.checked_add(len)?;
        if !self.readable.iter().any(|r| r.start <= at && end <= r.end) {
            return None;
        }
        self.bytes.get(at..end)
    }
    pub fn u16(&self, at: usize) -> Option<u16> {
        Some(u16::from_le_bytes(self.read(at, 2)?.try_into().ok()?))
    }
    pub fn u32(&self, at: usize) -> Option<u32> {
        Some(u32::from_le_bytes(self.read(at, 4)?.try_into().ok()?))
    }
    pub fn u64(&self, at: usize) -> Option<u64> {
        Some(u64::from_le_bytes(self.read(at, 8)?.try_into().ok()?))
    }
    pub fn rva(&self, ptr: usize) -> Option<usize> {
        ptr.checked_sub(self.base).filter(|&r| r < self.bytes.len())
    }
    pub fn cstr(&self, at: usize) -> Option<&str> {
        let available = self.readable.iter().find(|r| r.contains(&at))?.end - at;
        let bytes = self.read(at, available.min(256))?;
        let end = bytes.iter().position(|&b| b == 0)?;
        let text = std::str::from_utf8(&bytes[..end]).ok()?;
        (!text.is_empty() && text.bytes().all(|b| (32..127).contains(&b))).then_some(text)
    }
    pub fn executable(&self, at: usize) -> bool {
        self.sections
            .iter()
            .any(|s| s.executable && s.range.contains(&at))
            && self.read(at, 1).is_some()
    }
    pub fn data(&self, at: usize, len: usize, writable: bool) -> bool {
        self.read(at, len).is_some()
            && self.sections.iter().any(|s| {
                !s.executable
                    && (!writable || s.writable)
                    && s.range.start <= at
                    && at + len <= s.range.end
            })
    }
    pub fn string_hits(&self, text: &str) -> Vec<usize> {
        let needle = [text.as_bytes(), &[0]].concat();
        self.sections
            .iter()
            .filter(|s| !s.executable)
            .flat_map(|s| {
                self.bytes[s.range.clone()]
                    .windows(needle.len())
                    .enumerate()
                    .filter(|(i, b)| {
                        *b == needle && self.read(s.range.start + i, needle.len()).is_some()
                    })
                    .map(|(i, _)| s.range.start + i)
                    .collect::<Vec<_>>()
            })
            .collect()
    }
    pub fn is_slinky(&self) -> bool {
        [
            "Aim Assist",
            "Block In",
            "accent_color",
            "Press %s to open GUI",
        ]
        .iter()
        .all(|name| !self.string_hits(name).is_empty())
    }
}
