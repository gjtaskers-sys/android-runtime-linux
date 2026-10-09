//! Minimal APK inspection without Android SDK dependencies.
//!
//! We intentionally inspect the ZIP central directory ourselves instead of
//! shelling out to `aapt2`. That keeps the desktop runtime small and lets the
//! installer run on a clean Debian/Ubuntu/Mint machine.

use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Abi {
    Arm64,
    Arm32,
    X86_64,
    X86,
    Riscv64,
    Unknown,
}

impl Abi {
    pub fn from_path(path: &str) -> Option<Self> {
        let mut parts = path.split('/');
        if parts.next()? != "lib" {
            return None;
        }
        let abi = parts.next()?;
        let filename = parts.next()?;
        if parts.next().is_some() || !filename.ends_with(".so") || filename.len() <= 3 {
            return None;
        }
        match abi {
            "arm64-v8a" => Some(Self::Arm64),
            "armeabi-v7a" | "armeabi" => Some(Self::Arm32),
            "x86_64" => Some(Self::X86_64),
            "x86" => Some(Self::X86),
            "riscv64" => Some(Self::Riscv64),
            _ => None,
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Arm64 => "arm64-v8a",
            Self::Arm32 => "armeabi-v7a/armeabi",
            Self::X86_64 => "x86_64",
            Self::X86 => "x86",
            Self::Riscv64 => "riscv64",
            Self::Unknown => "unknown",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApkInfo {
    pub path: PathBuf,
    pub abis: Vec<Abi>,
    pub has_native_code: bool,
    pub is_split_like: bool,
    pub package_name: Option<String>,
}

impl ApkInfo {
    pub fn supports_host_isa(&self, host_arch: &str) -> bool {
        match host_arch {
            "x86_64" => self.abis.iter().any(|a| *a == Abi::X86_64 || *a == Abi::X86),
            "aarch64" => self.abis.iter().any(|a| *a == Abi::Arm64 || *a == Abi::Arm32),
            "x86" => self.abis.iter().any(|a| *a == Abi::X86),
            "arm" | "armv7" => self.abis.iter().any(|a| *a == Abi::Arm32),
            _ => false,
        }
    }

    pub fn needs_arm_translation_on_x86(&self) -> bool {
        !self.supports_host_isa("x86_64")
            && self.abis.iter().any(|a| *a == Abi::Arm64 || *a == Abi::Arm32)
    }
}

fn read_u16(data: &[u8], off: usize) -> io::Result<u16> {
    let end = off.checked_add(2).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "AXML offset overflow"))?;
    let bytes = data.get(off..end).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "AXML truncated"))?;
    Ok(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(data: &[u8], off: usize) -> io::Result<u32> {
    let end = off.checked_add(4).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "AXML offset overflow"))?;
    let bytes = data.get(off..end).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "AXML truncated"))?;
    Ok(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn decode_axml_length(data: &[u8], off: usize) -> io::Result<(usize, usize)> {
    let first = read_u16(data, off)?;
    if first & 0x8000 == 0 {
        Ok((first as usize, off + 2))
    } else {
        let second = read_u16(data, off + 2)?;
        Ok((((first & 0x7fff) as usize) << 15 | second as usize, off + 4))
    }
}

fn decode_axml_utf8_length(data: &[u8], off: usize) -> io::Result<(usize, usize)> {
    let first = *data.get(off).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "AXML UTF-8 length truncated"))?;
    if first & 0x80 == 0 {
        Ok((first as usize, off + 1))
    } else {
        let second = *data.get(off + 1).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "AXML UTF-8 length truncated"))?;
        Ok((((first & 0x7f) as usize) << 8 | second as usize, off + 2))
    }
}

fn string_pool_value(
    data: &[u8],
    pool_start: usize,
    string_offsets: &[u32],
    index: usize,
    utf8: bool,
) -> io::Result<String> {
    let offset = *string_offsets
        .get(index)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "AXML string index out of range"))? as usize;
    let start = pool_start
        .checked_add(offset)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "AXML string offset overflow"))?;

    if utf8 {
        // UTF-8 pools carry both UTF-16-unit length and UTF-8-byte length.
        let (_, pos) = decode_axml_utf8_length(data, start)?;
        let (byte_len, pos) = decode_axml_utf8_length(data, pos)?;
        let end = pos
            .checked_add(byte_len)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "AXML UTF-8 string overflow"))?;
        let bytes = data
            .get(pos..end)
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "AXML UTF-8 string truncated"))?;
        return String::from_utf8(bytes.to_vec())
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "AXML string is not valid UTF-8"));
    }

    let (units, pos) = decode_axml_length(data, start)?;
    let byte_len = units
        .checked_mul(2)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "AXML string too long"))?;
    let end = pos
        .checked_add(byte_len)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "AXML string overflow"))?;
    let bytes = data
        .get(pos..end)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "AXML string truncated"))?;
    let mut words = Vec::with_capacity(units);
    for chunk in bytes.chunks_exact(2) {
        words.push(u16::from_le_bytes([chunk[0], chunk[1]]));
    }
    String::from_utf16(&words)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "AXML string is not valid UTF-16"))
}

fn parse_manifest_package(bytes: &[u8]) -> io::Result<Option<String>> {
    if bytes.len() < 8 || read_u16(bytes, 0)? != 0x0003 {
        return Ok(None);
    }
    let declared_size = read_u32(bytes, 4)? as usize;
    if declared_size < 8 || declared_size > bytes.len() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid AXML document size"));
    }
    let bytes = &bytes[..declared_size];

    let mut off = 8usize;
    let mut pool_start = None;
    let mut string_offsets = Vec::<u32>::new();
    let mut pool_utf8 = false;

    while off + 8 <= bytes.len() {
        let chunk_type = read_u16(bytes, off)?;
        let header_size = read_u16(bytes, off + 2)? as usize;
        let chunk_size = read_u32(bytes, off + 4)? as usize;
        if header_size < 8 || chunk_size < header_size || off.checked_add(chunk_size).map_or(true, |e| e > bytes.len()) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid AXML chunk"));
        }
        match chunk_type {
            0x0001 => {
                if header_size < 28 { return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid AXML string pool")); }
                let string_count = read_u32(bytes, off + 8)? as usize;
                let flags = read_u32(bytes, off + 16)?;
                let strings_start = read_u32(bytes, off + 20)? as usize;
                let offsets_start = off + header_size;
                let offsets_bytes = string_count.checked_mul(4).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "AXML string pool overflow"))?;
                let offsets_end = offsets_start.checked_add(offsets_bytes)
                    .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "AXML string offsets overflow"))?;
                let chunk_end = off + chunk_size;
                if offsets_end > chunk_end {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "truncated AXML string offsets"));
                }
                if strings_start < header_size + offsets_bytes || strings_start > chunk_size {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid AXML string data offset"));
                }
                string_offsets.clear();
                for i in 0..string_count { string_offsets.push(read_u32(bytes, offsets_start + i * 4)?); }
                pool_start = Some(off + strings_start);
                pool_utf8 = flags & 0x100 != 0;
            }
            0x0102 => {
                let Some(pool) = pool_start else { off += chunk_size; continue; };
                if header_size < 16 || chunk_size < 16 + 20 {
                    return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid AXML start element"));
                }
                let name_index = read_u32(bytes, off + 20)? as usize;
                let name = string_pool_value(bytes, pool, &string_offsets, name_index, pool_utf8)?;
                let attr_start = read_u16(bytes, off + 16 + 8)? as usize;
                let attr_size = read_u16(bytes, off + 16 + 10)? as usize;
                let attr_count = read_u16(bytes, off + 16 + 12)? as usize;
                if attr_size < 20 { return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid AXML attribute size")); }
                let attrs_base = off + 16 + attr_start;
                if name == "manifest" {
                    for i in 0..attr_count {
                        let a = attrs_base.checked_add(i.checked_mul(attr_size).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "AXML attribute overflow"))?).ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "AXML attribute overflow"))?;
                        if a + 20 > off + chunk_size { return Err(io::Error::new(io::ErrorKind::InvalidData, "truncated AXML attribute")); }
                        let attr_name_index = read_u32(bytes, a + 4)? as usize;
                        // ResXMLTree_attribute has a 4-byte rawValue before its
                        // typed Res_value. The size field is at +12, not +8.
                        let raw_value = read_u32(bytes, a + 8)?;
                        let value_size = read_u16(bytes, a + 12)?;
                        let value_type = bytes.get(a + 15).copied().ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "truncated AXML value"))?;
                        let value_data = read_u32(bytes, a + 16)? as usize;
                        if value_size < 8 { continue; }
                        let attr_name = string_pool_value(bytes, pool, &string_offsets, attr_name_index, pool_utf8)?;
                        if attr_name == "package" {
                            if value_type == 0x03 {
                                return string_pool_value(bytes, pool, &string_offsets, value_data, pool_utf8).map(Some);
                            }
                            // Some toolchains preserve the lexical string in
                            // rawValue even when the typed value is not a string.
                            if raw_value != u32::MAX {
                                return string_pool_value(bytes, pool, &string_offsets, raw_value as usize, pool_utf8).map(Some);
                            }
                        }
                    }
                }
            }
            _ => {}
        }
        off += chunk_size;
    }
    Ok(None)
}

const MAX_MANIFEST_BYTES: u32 = 8 * 1024 * 1024;
// Keep malformed ZIP metadata from causing a multi-gigabyte allocation.
const MAX_CENTRAL_DIRECTORY_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy)]
struct ManifestEntry {
    local_offset: u32,
    flags: u16,
    method: u16,
    crc32: u32,
    compressed_size: u32,
    uncompressed_size: u32,
}

#[derive(Debug, Clone, Copy)]
struct ArchiveBounds {
    central_directory_offset: u64,
    archive_len: u64,
}

fn zip_crc32(bytes: &[u8]) -> u32 {
    let mut crc = !0u32;
    for byte in bytes {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = if crc & 1 != 0 { (crc >> 1) ^ 0xedb8_8320 } else { crc >> 1 };
        }
    }
    !crc
}

/// Read AndroidManifest.xml from a ZIP entry. APK manifests are commonly stored,
/// but valid APKs may also DEFLATE them. For DEFLATE, use Info-ZIP's `unzip` as a
/// small, widely available system helper instead of adding a Rust compression
/// dependency to the host runtime.
fn read_manifest_entry(
    path: &Path,
    file: &mut File,
    entry: ManifestEntry,
    bounds: ArchiveBounds,
) -> io::Result<Option<Vec<u8>>> {
    if entry.uncompressed_size > MAX_MANIFEST_BYTES {
        return Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("AndroidManifest.xml exceeds the {} MiB safety limit", MAX_MANIFEST_BYTES / (1024 * 1024)),
        ));
    }

    match entry.method {
        0 => {
            if entry.compressed_size != entry.uncompressed_size {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "stored AndroidManifest.xml has inconsistent ZIP sizes"));
            }
            let local = entry.local_offset as u64;
            file.seek(SeekFrom::Start(local))?;
            let mut hdr = [0u8; 30];
            file.read_exact(&mut hdr)?;
            if hdr[0..4] != [0x50, 0x4b, 0x03, 0x04] {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid local ZIP header for AndroidManifest.xml"));
            }
            let local_flags = u16::from_le_bytes([hdr[6], hdr[7]]);
            let local_method = u16::from_le_bytes([hdr[8], hdr[9]]);
            if local_method != entry.method || local_flags != entry.flags {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "ZIP local header flags or compression method mismatch for AndroidManifest.xml"));
            }
            let local_compressed = u32::from_le_bytes([hdr[18], hdr[19], hdr[20], hdr[21]]);
            let local_uncompressed = u32::from_le_bytes([hdr[22], hdr[23], hdr[24], hdr[25]]);
            if entry.flags & 0x0008 == 0 && (local_compressed != entry.compressed_size || local_uncompressed != entry.uncompressed_size) {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "ZIP local header sizes disagree with the central directory"));
            }
            let name_len = u16::from_le_bytes([hdr[26], hdr[27]]) as usize;
            let extra_len = u16::from_le_bytes([hdr[28], hdr[29]]) as usize;
            let data_start = local.checked_add(30)
                .and_then(|v| v.checked_add(name_len as u64))
                .and_then(|v| v.checked_add(extra_len as u64))
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "ZIP local header offset overflow"))?;
            let data_end = data_start.checked_add(entry.compressed_size as u64)
                .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "ZIP manifest data offset overflow"))?;
            if data_end > bounds.central_directory_offset || data_end > bounds.archive_len {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "AndroidManifest.xml data extends beyond the ZIP entry area"));
            }
            let mut local_name = vec![0u8; name_len];
            file.seek(SeekFrom::Start(local + 30))?;
            file.read_exact(&mut local_name)?;
            if local_name != b"AndroidManifest.xml" {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "ZIP local header name disagrees with AndroidManifest.xml central entry"));
            }
            file.seek(SeekFrom::Start(data_start))?;
            let mut manifest = vec![0u8; entry.uncompressed_size as usize];
            file.read_exact(&mut manifest)?;
            if zip_crc32(&manifest) != entry.crc32 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "AndroidManifest.xml CRC does not match the ZIP central directory"));
            }
            Ok(Some(manifest))
        }
        8 => {
            let archive = path.canonicalize()?;
            let mut child = Command::new("unzip")
                .arg("-p")
                .arg(archive)
                .arg("AndroidManifest.xml")
                .stdout(Stdio::piped())
                .stderr(Stdio::null())
                .spawn()
                .map_err(|e| {
                    if e.kind() == io::ErrorKind::NotFound {
                        io::Error::new(io::ErrorKind::NotFound, "this APK compresses AndroidManifest.xml; install Info-ZIP with `sudo apt install unzip` and retry")
                    } else {
                        e
                    }
                })?;
            let stdout = child.stdout.take().ok_or_else(|| io::Error::new(io::ErrorKind::Other, "could not read unzip output"))?;
            let mut limited = stdout.take(MAX_MANIFEST_BYTES as u64 + 1);
            let mut manifest = Vec::new();
            if let Err(e) = limited.read_to_end(&mut manifest) {
                let _ = child.kill();
                let _ = child.wait();
                return Err(e);
            }
            drop(limited);
            if manifest.len() > MAX_MANIFEST_BYTES as usize {
                let _ = child.kill();
                let _ = child.wait();
                return Err(io::Error::new(io::ErrorKind::InvalidData, "decompressed AndroidManifest.xml exceeds the 8 MiB safety limit"));
            }
            let status = child.wait()?;
            if !status.success() {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "unzip could not extract AndroidManifest.xml from this APK"));
            }
            if manifest.len() != entry.uncompressed_size as usize {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "AndroidManifest.xml size does not match the ZIP central directory"));
            }
            if zip_crc32(&manifest) != entry.crc32 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "AndroidManifest.xml CRC does not match the ZIP central directory"));
            }
            Ok(Some(manifest))
        }
        other => Err(io::Error::new(
            io::ErrorKind::InvalidData,
            format!("unsupported ZIP compression method {other} for AndroidManifest.xml"),
        )),
    }
}

/// Read enough of the ZIP central directory to enumerate file names and locate
/// AndroidManifest.xml. ABI discovery comes from central-directory names; the
/// manifest is extracted separately with a bounded size limit.
pub fn inspect(path: impl AsRef<Path>) -> io::Result<ApkInfo> {
    let path = path.as_ref().to_path_buf();
    let mut file = File::open(&path)?;
    let len = file.metadata()?.len();
    if len < 22 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "not a ZIP/APK"));
    }

    let tail_len = len.min(1024 * 1024) as usize;
    file.seek(SeekFrom::End(-(tail_len as i64)))?;
    let mut tail = vec![0_u8; tail_len];
    file.read_exact(&mut tail)?;

    // An EOCD signature can occur inside the ZIP comment. Only accept a
    // candidate whose declared comment ends exactly at EOF.
    let mut eocd = None;
    for i in (0..=tail_len.saturating_sub(22)).rev() {
        if tail[i..i + 4] != [0x50, 0x4b, 0x05, 0x06] {
            continue;
        }
        let comment_len = u16::from_le_bytes([tail[i + 20], tail[i + 21]]) as usize;
        if i.checked_add(22).and_then(|v| v.checked_add(comment_len)) == Some(tail.len()) {
            eocd = Some(i);
            break;
        }
    }
    let eocd = eocd.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "ZIP EOCD not found or invalid comment length"))?;
    let tail_start = len - tail_len as u64;
    let eocd_offset = tail_start + eocd as u64;

    let disk_number = u16::from_le_bytes([tail[eocd + 4], tail[eocd + 5]]);
    let cd_disk = u16::from_le_bytes([tail[eocd + 6], tail[eocd + 7]]);
    let disk_entries = u16::from_le_bytes([tail[eocd + 8], tail[eocd + 9]]);
    let total_entries = u16::from_le_bytes([tail[eocd + 10], tail[eocd + 11]]);
    let cd_size = u32::from_le_bytes([
        tail[eocd + 12], tail[eocd + 13], tail[eocd + 14], tail[eocd + 15],
    ]) as u64;
    let cd_offset = u32::from_le_bytes([
        tail[eocd + 16], tail[eocd + 17], tail[eocd + 18], tail[eocd + 19],
    ]) as u64;

    if disk_number != 0 || cd_disk != 0 || disk_entries != total_entries {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "multi-disk ZIP archives are not supported"));
    }
    if total_entries == u16::MAX || cd_size == u32::MAX as u64 || cd_offset == u32::MAX as u64 {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "ZIP64 APKs are not supported by this inspector"));
    }
    if cd_size > MAX_CENTRAL_DIRECTORY_BYTES {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "ZIP central directory exceeds the 64 MiB safety limit"));
    }
    let cd_end = cd_offset.checked_add(cd_size)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "ZIP central directory offset overflow"))?;
    if cd_end > eocd_offset || cd_end > len {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "ZIP central directory points outside the archive"));
    }
    if total_entries == 0 {
        if cd_size != 0 {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "empty ZIP has a non-empty central directory"));
        }
        return Ok(ApkInfo {
            path,
            abis: Vec::new(),
            has_native_code: false,
            is_split_like: false,
            package_name: None,
        });
    }

    file.seek(SeekFrom::Start(cd_offset))?;
    let mut cd = vec![0_u8; cd_size as usize];
    file.read_exact(&mut cd)?;

    let mut offset = 0usize;
    let mut abis = Vec::new();
    let mut has_native_code = false;
    let mut is_split_like = false;
    let mut manifest_entry: Option<ManifestEntry> = None;

    for _ in 0..total_entries {
        if offset.checked_add(46).map_or(true, |end| end > cd.len()) {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "truncated ZIP central directory entry"));
        }
        if cd[offset..offset + 4] != [0x50, 0x4b, 0x01, 0x02] {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "invalid ZIP central directory signature"));
        }
        let name_len = u16::from_le_bytes([cd[offset + 28], cd[offset + 29]]) as usize;
        let extra_len = u16::from_le_bytes([cd[offset + 30], cd[offset + 31]]) as usize;
        let comment_len = u16::from_le_bytes([cd[offset + 32], cd[offset + 33]]) as usize;
        let end = offset
            .checked_add(46)
            .and_then(|v| v.checked_add(name_len))
            .and_then(|v| v.checked_add(extra_len))
            .and_then(|v| v.checked_add(comment_len))
            .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "ZIP record overflow"))?;
        if end > cd.len() {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "truncated ZIP central directory record"));
        }
        let name = String::from_utf8_lossy(&cd[offset + 46..offset + 46 + name_len]);
        if name.starts_with("lib/") && name.ends_with(".so") {
            has_native_code = true;
            if let Some(abi) = Abi::from_path(&name) {
                if !abis.contains(&abi) {
                    abis.push(abi);
                }
            }
        }
        if name == "AndroidManifest.xml" {
            if manifest_entry.is_some() {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "APK contains duplicate AndroidManifest.xml entries"));
            }
            let flags = u16::from_le_bytes([cd[offset + 8], cd[offset + 9]]);
            if flags & 0x0001 != 0 || flags & 0x0040 != 0 {
                return Err(io::Error::new(io::ErrorKind::InvalidData, "encrypted AndroidManifest.xml entries are not supported"));
            }
            let method = u16::from_le_bytes([cd[offset + 10], cd[offset + 11]]);
            let crc32 = u32::from_le_bytes([cd[offset + 16], cd[offset + 17], cd[offset + 18], cd[offset + 19]]);
            let compressed_size = u32::from_le_bytes([cd[offset + 20], cd[offset + 21], cd[offset + 22], cd[offset + 23]]);
            let uncompressed_size = u32::from_le_bytes([cd[offset + 24], cd[offset + 25], cd[offset + 26], cd[offset + 27]]);
            let local_offset = u32::from_le_bytes([cd[offset + 42], cd[offset + 43], cd[offset + 44], cd[offset + 45]]);
            manifest_entry = Some(ManifestEntry {
                local_offset,
                flags,
                method,
                crc32,
                compressed_size,
                uncompressed_size,
            });
        }
        if name.contains("split_config.") || name.starts_with("splits/") {
            is_split_like = true;
        }
        offset = end;
    }

    if offset != cd.len() {
        return Err(io::Error::new(io::ErrorKind::InvalidData, "unexpected data after ZIP central directory entries"));
    }

    let package_name = match manifest_entry {
        Some(entry) => match read_manifest_entry(
            &path,
            &mut file,
            entry,
            ArchiveBounds {
                central_directory_offset: cd_offset,
                archive_len: len,
            },
        )? {
            Some(manifest) => parse_manifest_package(&manifest)?,
            None => None,
        },
        None => None,
    };

    Ok(ApkInfo {
        path,
        abis,
        has_native_code,
        is_split_like,
        package_name,
    })
}


#[cfg(test)]
mod tests {
    use super::*;

    fn put_u16(v: &mut Vec<u8>, x: u16) { v.extend_from_slice(&x.to_le_bytes()); }
    fn put_u32(v: &mut Vec<u8>, x: u32) { v.extend_from_slice(&x.to_le_bytes()); }

    fn utf8_len(n: usize) -> Vec<u8> {
        if n < 0x80 { vec![n as u8] } else { vec![((n >> 8) as u8) | 0x80, (n & 0xff) as u8] }
    }

    #[test]
    fn rejects_binary_manifest_with_invalid_document_size() {
        let mut xml = vec![0u8; 8];
        xml[0..2].copy_from_slice(&0x0003u16.to_le_bytes());
        xml[2..4].copy_from_slice(&8u16.to_le_bytes());
        xml[4..8].copy_from_slice(&u32::MAX.to_le_bytes());
        assert!(parse_manifest_package(&xml).is_err());
    }

    #[test]
    fn parses_utf8_binary_manifest_package_name() {
        let strings = ["manifest", "package", "com.example.runtime_test"];
        let mut pool_data = Vec::new();
        let mut offsets = Vec::new();
        for value in strings {
            offsets.push(pool_data.len() as u32);
            let utf8 = value.as_bytes();
            let utf16_units = value.encode_utf16().count();
            pool_data.extend(utf8_len(utf16_units));
            pool_data.extend(utf8_len(utf8.len()));
            pool_data.extend_from_slice(utf8);
            pool_data.push(0);
        }
        while pool_data.len() % 4 != 0 { pool_data.push(0); }

        let string_count = strings.len() as u32;
        let string_offsets_size = string_count * 4;
        let strings_start = 28 + string_offsets_size;
        let pool_size = strings_start + pool_data.len() as u32;
        let mut pool = Vec::new();
        put_u16(&mut pool, 0x0001);
        put_u16(&mut pool, 28);
        put_u32(&mut pool, pool_size);
        put_u32(&mut pool, string_count);
        put_u32(&mut pool, 0);
        put_u32(&mut pool, 0x100);
        put_u32(&mut pool, strings_start);
        put_u32(&mut pool, 0);
        for offset in offsets { put_u32(&mut pool, offset); }
        pool.extend_from_slice(&pool_data);

        let mut elem = Vec::new();
        put_u16(&mut elem, 0x0102);
        put_u16(&mut elem, 16);
        put_u32(&mut elem, 16 + 20 + 20);
        put_u32(&mut elem, 1);
        put_u32(&mut elem, 0xffff_ffff);
        put_u32(&mut elem, 0xffff_ffff); // namespace
        put_u32(&mut elem, 0); // name -> manifest
        put_u16(&mut elem, 20); // attributeStart
        put_u16(&mut elem, 20); // attributeSize
        put_u16(&mut elem, 1); // attributeCount
        put_u16(&mut elem, 0); // idIndex
        put_u16(&mut elem, 0); // classIndex
        put_u16(&mut elem, 0); // styleIndex
        put_u32(&mut elem, 0xffff_ffff); // attr namespace
        put_u32(&mut elem, 1); // attr name -> package
        put_u32(&mut elem, 2); // rawValue -> package string
        put_u16(&mut elem, 8); // typed value size
        elem.push(0); // res0
        elem.push(0x03); // TYPE_STRING
        put_u32(&mut elem, 2); // string index

        let total = 8 + pool.len() + elem.len();
        let mut xml = Vec::new();
        put_u16(&mut xml, 0x0003);
        put_u16(&mut xml, 8);
        put_u32(&mut xml, total as u32);
        xml.extend_from_slice(&pool);
        xml.extend_from_slice(&elem);

        assert_eq!(parse_manifest_package(&xml).unwrap().as_deref(), Some("com.example.runtime_test"));
    }

    fn append_u16(v: &mut Vec<u8>, x: u16) { v.extend_from_slice(&x.to_le_bytes()); }
    fn append_u32(v: &mut Vec<u8>, x: u32) { v.extend_from_slice(&x.to_le_bytes()); }

    fn tiny_stored_apk() -> Vec<u8> {
        // Reuse the AXML fixture generated above, then wrap it in a single-entry
        // stored ZIP so inspect() exercises both ZIP parsing and manifest parsing.
        let strings = ["manifest", "package", "com.example.zip_test"];
        let mut pool_data = Vec::new();
        let mut offsets = Vec::new();
        for value in strings {
            offsets.push(pool_data.len() as u32);
            let utf8 = value.as_bytes();
            let utf16_units = value.encode_utf16().count();
            pool_data.extend(utf8_len(utf16_units));
            pool_data.extend(utf8_len(utf8.len()));
            pool_data.extend_from_slice(utf8);
            pool_data.push(0);
        }
        while pool_data.len() % 4 != 0 { pool_data.push(0); }
        let string_count = strings.len() as u32;
        let strings_start = 28 + string_count * 4;
        let mut pool = Vec::new();
        append_u16(&mut pool, 0x0001); append_u16(&mut pool, 28);
        append_u32(&mut pool, strings_start + pool_data.len() as u32);
        append_u32(&mut pool, string_count); append_u32(&mut pool, 0); append_u32(&mut pool, 0x100);
        append_u32(&mut pool, strings_start); append_u32(&mut pool, 0);
        for offset in offsets { append_u32(&mut pool, offset); }
        pool.extend_from_slice(&pool_data);

        let mut elem = Vec::new();
        append_u16(&mut elem, 0x0102); append_u16(&mut elem, 16); append_u32(&mut elem, 56);
        append_u32(&mut elem, 1); append_u32(&mut elem, 0xffff_ffff); append_u32(&mut elem, 0xffff_ffff);
        append_u32(&mut elem, 0); append_u16(&mut elem, 20); append_u16(&mut elem, 20);
        append_u16(&mut elem, 1); append_u16(&mut elem, 0); append_u16(&mut elem, 0); append_u16(&mut elem, 0);
        append_u32(&mut elem, 0xffff_ffff); append_u32(&mut elem, 1); append_u32(&mut elem, 2); append_u16(&mut elem, 8); elem.push(0); elem.push(0x03);
        append_u32(&mut elem, 2);
        let mut manifest = Vec::new();
        append_u16(&mut manifest, 0x0003); append_u16(&mut manifest, 8);
        append_u32(&mut manifest, (8 + pool.len() + elem.len()) as u32);
        manifest.extend_from_slice(&pool); manifest.extend_from_slice(&elem);

        let filename = b"AndroidManifest.xml";
        let local_size = 30 + filename.len() + manifest.len();
        let cd_offset = local_size as u32;
        let cd_size = (46 + filename.len()) as u32;
        // `local_size` is a host usize while ZIP field sizes are u32. Keep
        // the test fixture's capacity calculation in usize and fail clearly
        // if the fixture ever becomes unreasonably large.
        let total = local_size
            .checked_add(cd_size as usize)
            .and_then(|size| size.checked_add(22))
            .expect("test APK ZIP size overflow");
        let mut zip = Vec::with_capacity(total);
        append_u32(&mut zip, 0x04034b50); append_u16(&mut zip, 20); append_u16(&mut zip, 0);
        append_u16(&mut zip, 0); append_u16(&mut zip, 0); append_u16(&mut zip, 0); append_u32(&mut zip, zip_crc32(&manifest));
        append_u32(&mut zip, manifest.len() as u32); append_u32(&mut zip, manifest.len() as u32);
        append_u16(&mut zip, filename.len() as u16); append_u16(&mut zip, 0);
        zip.extend_from_slice(filename); zip.extend_from_slice(&manifest);
        append_u32(&mut zip, 0x02014b50); append_u16(&mut zip, 20); append_u16(&mut zip, 20);
        append_u16(&mut zip, 0); append_u16(&mut zip, 0); append_u16(&mut zip, 0); append_u16(&mut zip, 0);
        append_u32(&mut zip, zip_crc32(&manifest)); append_u32(&mut zip, manifest.len() as u32); append_u32(&mut zip, manifest.len() as u32);
        append_u16(&mut zip, filename.len() as u16); append_u16(&mut zip, 0); append_u16(&mut zip, 0); append_u16(&mut zip, 0); append_u16(&mut zip, 0);
        append_u32(&mut zip, 0); append_u32(&mut zip, 0);
        zip.extend_from_slice(filename);
        append_u32(&mut zip, 0x06054b50); append_u16(&mut zip, 0); append_u16(&mut zip, 0);
        append_u16(&mut zip, 1); append_u16(&mut zip, 1); append_u32(&mut zip, cd_size); append_u32(&mut zip, cd_offset);
        append_u16(&mut zip, 0);
        zip
    }

    #[test]
    fn inspect_reads_package_from_stored_apk() {
        let dir = std::env::temp_dir().join(format!("android-runtime-apk-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("test.apk");
        std::fs::write(&path, tiny_stored_apk()).unwrap();
        let info = inspect(&path).unwrap();
        assert_eq!(info.package_name.as_deref(), Some("com.example.zip_test"));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn rejects_truncated_zip_without_panicking() {
        let dir = std::env::temp_dir().join(format!("android-runtime-apk-truncated-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("truncated.apk");
        std::fs::write(&path, [0u8; 22]).unwrap();
        assert!(inspect(&path).is_err());
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn rejects_manifest_with_wrong_zip_crc() {
        let dir = std::env::temp_dir().join(format!("android-runtime-apk-bad-crc-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad-crc.apk");
        let mut bytes = tiny_stored_apk();
        let cd = bytes.windows(4).position(|window| window == [0x50, 0x4b, 0x01, 0x02]).unwrap();
        bytes[cd + 16..cd + 20].copy_from_slice(&0x1234_5678u32.to_le_bytes());
        std::fs::write(&path, bytes).unwrap();
        assert!(inspect(&path).unwrap_err().to_string().contains("CRC"));
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn rejects_central_directory_outside_archive() {
        let dir = std::env::temp_dir().join(format!("android-runtime-apk-bad-offset-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("bad-offset.apk");
        let mut bytes = tiny_stored_apk();
        let eocd = bytes.len() - 22;
        bytes[eocd + 16..eocd + 20].copy_from_slice(&u32::MAX.to_le_bytes());
        std::fs::write(&path, bytes).unwrap();
        assert!(inspect(&path).is_err());
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_dir(&dir);
    }

    #[test]
    fn abi_detection_rejects_non_library_paths() {
        assert_eq!(Abi::from_path("lib/arm64-v8a/libsample.so"), Some(Abi::Arm64));
        assert_eq!(Abi::from_path("lib/arm64-v8a/nested/libsample.so"), None);
        assert_eq!(Abi::from_path("lib/arm64-v8a/README.txt"), None);
    }

}
