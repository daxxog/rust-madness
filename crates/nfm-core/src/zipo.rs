//! Minimal reader for the game's `.zipo` container — these are plain zip
//! archives the applet streams through `java.util.zip.ZipInputStream`.
//!
//! We only need read-only access to the members of small, archive-built
//! (non-ZIP64) files, so this intentionally stays lean rather than pulling
//! in a full zip crate: locate the End-of-Central-Directory record, walk the
//! central directory, slice each member's data and inflate it when needed.

use std::fmt;

#[derive(Debug, Clone)]
pub enum ZipoError {
    TooShort,
    EocdNotFound,
    BadEocd,
    TruncatedCentralDirectory {
        at: usize,
    },
    BadCentralEntry {
        at: usize,
    },
    BadLocalHeader {
        name: String,
    },
    UnsupportedMethod {
        name: String,
        method: u16,
    },
    DeflateFailure {
        name: String,
    },
    SizeMismatch {
        name: String,
        expected: usize,
        got: usize,
    },
}

impl fmt::Display for ZipoError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ZipoError::TooShort => write!(f, "file too small to be a zip"),
            ZipoError::EocdNotFound => write!(f, "end-of-central-directory record not found"),
            ZipoError::BadEocd => write!(f, "malformed end-of-central-directory record"),
            ZipoError::TruncatedCentralDirectory { at } => {
                write!(f, "central directory truncated at {at}")
            }
            ZipoError::BadCentralEntry { at } => write!(f, "bad central directory entry at {at}"),
            ZipoError::BadLocalHeader { name } => {
                write!(f, "bad local header for member {name:?}")
            }
            ZipoError::UnsupportedMethod { name, method } => {
                write!(
                    f,
                    "member {name:?} uses unsupported compression method {method}"
                )
            }
            ZipoError::DeflateFailure { name } => write!(f, "deflate failed for member {name:?}"),
            ZipoError::SizeMismatch {
                name,
                expected,
                got,
            } => {
                write!(
                    f,
                    "member {name:?} inflated to {got} bytes, central dir says {expected}"
                )
            }
        }
    }
}

impl std::error::Error for ZipoError {}

pub struct ZipoEntry {
    pub name: String,
    pub data: Vec<u8>,
}

pub struct Zipo {
    /// Members in central-directory order (the order the original
    /// `ZipInputStream.getNextEntry()` would yield them).
    pub entries: Vec<ZipoEntry>,
}

fn u16_at(b: &[u8], at: usize) -> u16 {
    u16::from_le_bytes([b[at], b[at + 1]])
}

fn u32_at(b: &[u8], at: usize) -> u32 {
    u32::from_le_bytes([b[at], b[at + 1], b[at + 2], b[at + 3]])
}

impl Zipo {
    pub fn read(bytes: &[u8]) -> Result<Zipo, ZipoError> {
        if bytes.len() < 22 {
            return Err(ZipoError::TooShort);
        }

        // EOCD: scan backwards over the (max 64 KiB) archive comment.
        let mut eocd_at = None;
        for start in (0..bytes.len().saturating_sub(21)).rev() {
            if bytes[start..start + 4] == [0x50, 0x4b, 0x05, 0x06] {
                eocd_at = Some(start);
                break;
            }
        }
        let eocd = eocd_at.ok_or(ZipoError::EocdNotFound)?;
        let e = &bytes[eocd..];
        if e.len() < 22 {
            return Err(ZipoError::BadEocd);
        }
        let count = u16_at(e, 10) as usize;
        let cd_size = u32_at(e, 12) as usize;
        let cd_offset = u32_at(e, 16) as usize;
        let cd_end = cd_offset.checked_add(cd_size).ok_or(ZipoError::BadEocd)?;
        if cd_end > eocd {
            return Err(ZipoError::BadEocd);
        }

        let mut entries = Vec::with_capacity(count);
        let mut p = cd_offset;
        for _ in 0..count {
            let c = bytes
                .get(p..)
                .ok_or(ZipoError::TruncatedCentralDirectory { at: p })?;
            if c.len() < 46 || c[..4] != [0x50, 0x4b, 0x01, 0x02] {
                return Err(ZipoError::BadCentralEntry { at: p });
            }
            let method = u16_at(c, 10);
            let comp_size = u32_at(c, 20) as usize;
            let uncomp_size = u32_at(c, 24) as usize;
            let name_len = u16_at(c, 28) as usize;
            let extra_len = u16_at(c, 30) as usize;
            let comment_len = u16_at(c, 32) as usize;
            let local_at = u32_at(c, 42) as usize;
            let name = c
                .get(46..46 + name_len)
                .ok_or(ZipoError::BadCentralEntry { at: p })?;
            let name = String::from_utf8_lossy(name).into_owned();

            // Local file header: 30 bytes fixed; name/extra lengths at +26/+28.
            let l = bytes
                .get(local_at..)
                .ok_or(ZipoError::BadLocalHeader { name: name.clone() })?;
            if l.len() < 30 || l[..4] != [0x50, 0x4b, 0x03, 0x04] {
                return Err(ZipoError::BadLocalHeader { name: name.clone() });
            }
            let l_name_len = u16_at(l, 26) as usize;
            let l_extra_len = u16_at(l, 28) as usize;
            let data_at = local_at + 30 + l_name_len + l_extra_len;
            let comp = bytes
                .get(data_at..data_at + comp_size)
                .ok_or(ZipoError::BadLocalHeader { name: name.clone() })?;

            let data = match method {
                0 => comp.to_vec(),
                8 => {
                    use flate2::read::DeflateDecoder;
                    use std::io::Read;
                    let mut out = Vec::with_capacity(uncomp_size);
                    if DeflateDecoder::new(comp).read_to_end(&mut out).is_err() {
                        return Err(ZipoError::DeflateFailure { name: name.clone() });
                    }
                    out
                }
                other => {
                    return Err(ZipoError::UnsupportedMethod {
                        name: name.clone(),
                        method: other,
                    });
                }
            };
            if data.len() != uncomp_size {
                return Err(ZipoError::SizeMismatch {
                    name: name.clone(),
                    expected: uncomp_size,
                    got: data.len(),
                });
            }

            entries.push(ZipoEntry { name, data });
            p += 46 + name_len + extra_len + comment_len;
        }

        Ok(Zipo { entries })
    }

    pub fn get(&self, name: &str) -> Option<&ZipoEntry> {
        self.entries.iter().find(|e| e.name == name)
    }
}
