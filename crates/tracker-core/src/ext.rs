//! 浏览器扩展相关的纯逻辑：protobuf varint、CRX3 头部解析、扩展 ID 编码、manifest 版本读取。
//!
//! 这些函数原本嵌在 src-tauri/src/lib.rs 里、完全没有测试覆盖。它们不碰 Tauri、
//! 不碰全局状态，只做字节与文件解析，所以放到 core 里就能直接写单测。

use std::path::Path;

/// 读 protobuf varint（只用来跳过字段或取长度）。
pub fn read_varint(buf: &[u8], mut i: usize) -> Option<(u64, usize)> {
    let mut value: u64 = 0;
    let mut shift = 0u32;
    while i < buf.len() && shift < 64 {
        let b = buf[i];
        i += 1;
        value |= ((b & 0x7f) as u64) << shift;
        if b & 0x80 == 0 {
            return Some((value, i));
        }
        shift += 7;
    }
    None
}

/// CRX3 头部 → signed_header_data 里的 crx_id（= SHA256(公钥) 前 16 字节）。
/// 结构：magic "Cr24" | uint32 LE 版本(=3) | uint32 LE header 长度 | CrxFileHeader。
pub fn crx_id_bytes(crx: &[u8]) -> Option<Vec<u8>> {
    if crx.len() < 16 || &crx[0..4] != b"Cr24" {
        return None;
    }
    let version = u32::from_le_bytes([crx[4], crx[5], crx[6], crx[7]]);
    if version != 3 {
        return None;
    }
    let header_size = u32::from_le_bytes([crx[8], crx[9], crx[10], crx[11]]) as usize;
    if header_size == 0 || 12 + header_size > crx.len() {
        return None;
    }
    let header = &crx[12..12 + header_size];
    let mut i = 0usize;
    while i < header.len() {
        let (key, next) = read_varint(header, i)?;
        i = next;
        let field = key >> 3;
        match key & 7 {
            0 => {
                let (_, next) = read_varint(header, i)?;
                i = next;
            }
            1 => i += 8,
            2 => {
                let (len, next) = read_varint(header, i)?;
                i = next;
                let end = i.checked_add(len as usize)?;
                if end > header.len() {
                    return None;
                }
                if field == 10000 {
                    return signed_data_crx_id(&header[i..end]);
                }
                i = end;
            }
            5 => i += 4,
            _ => return None,
        }
    }
    None
}

/// 从 Crx3 SignedData 里取 crx_id（字段 1，长度必须正好 16）。
pub fn signed_data_crx_id(buf: &[u8]) -> Option<Vec<u8>> {
    let mut i = 0usize;
    while i < buf.len() {
        let (key, next) = read_varint(buf, i)?;
        i = next;
        let field = key >> 3;
        match key & 7 {
            0 => {
                let (_, next) = read_varint(buf, i)?;
                i = next;
            }
            1 => i += 8,
            2 => {
                let (len, next) = read_varint(buf, i)?;
                i = next;
                let end = i.checked_add(len as usize)?;
                if end > buf.len() {
                    return None;
                }
                if field == 1 && len == 16 {
                    return Some(buf[i..end].to_vec());
                }
                i = end;
            }
            5 => i += 4,
            _ => return None,
        }
    }
    None
}

/// 16 字节公钥哈希 → Chromium 扩展 ID（a-p 编码，32 字符）。
pub fn extension_id_from_hash(hash: &[u8]) -> String {
    const ALPHABET: &[u8; 16] = b"abcdefghijklmnop";
    let mut id = String::with_capacity(32);
    for b in hash {
        id.push(ALPHABET[(b >> 4) as usize] as char);
        id.push(ALPHABET[(b & 0x0f) as usize] as char);
    }
    id
}

/// 读扩展目录下 manifest.json 的 version，读不到就退回 "1.0.0"。
pub fn manifest_version(dir: &Path) -> String {
    std::fs::read(dir.join("manifest.json"))
        .ok()
        .and_then(|b| serde_json::from_slice::<serde_json::Value>(&b).ok())
        .and_then(|v| {
            v.get("version")
                .and_then(|x| x.as_str())
                .map(str::to_string)
        })
        .unwrap_or_else(|| "1.0.0".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 把一个 u64 编成 protobuf varint，测试里拼假 CRX 用。
    fn varint(mut v: u64) -> Vec<u8> {
        let mut out = Vec::new();
        loop {
            let mut b = (v & 0x7f) as u8;
            v >>= 7;
            if v != 0 {
                b |= 0x80;
            }
            out.push(b);
            if v == 0 {
                return out;
            }
        }
    }

    /// 拼一个 length-delimited 字段：key = (field << 3) | 2。
    fn field_bytes(field: u64, payload: &[u8]) -> Vec<u8> {
        let mut out = varint((field << 3) | 2);
        out.extend(varint(payload.len() as u64));
        out.extend_from_slice(payload);
        out
    }

    fn crx_with_header(header: &[u8]) -> Vec<u8> {
        let mut crx = b"Cr24".to_vec();
        crx.extend((3u32).to_le_bytes());
        crx.extend((header.len() as u32).to_le_bytes());
        crx.extend_from_slice(header);
        crx.extend_from_slice(&[0u8; 8]);
        crx
    }

    #[test]
    fn varint_单字节与多字节() {
        assert_eq!(read_varint(&[0x00], 0), Some((0, 1)));
        assert_eq!(read_varint(&[0x96, 0x01], 0), Some((150, 2)));
        assert_eq!(read_varint(&[0xff, 0xff, 0x03], 0), Some((0xffff, 3)));
    }

    #[test]
    fn varint_截断返回_none() {
        assert_eq!(read_varint(&[0x80], 0), None);
        assert_eq!(read_varint(&[0x80, 0x80], 0), None);
        assert_eq!(read_varint(&[], 0), None);
        // 续位一直不断：shift 越过 64 位后放弃
        assert_eq!(read_varint(&[0xff; 12], 0), None);
    }

    #[test]
    fn 扩展_id_按半字节编码() {
        assert_eq!(extension_id_from_hash(&[0x00, 0xff]), "aapp");
        // 真实用例是 16 字节哈希 → 32 字符，全部落在 a..p
        let hash: Vec<u8> = (0u8..16).collect();
        let id = extension_id_from_hash(&hash);
        assert_eq!(id.len(), 32);
        assert!(id.bytes().all(|b| (b'a'..=b'p').contains(&b)));
        assert!(id.starts_with("aaabacad"));
        assert!(id.ends_with("aoap"));
    }

    #[test]
    fn crx3_头部取到_crx_id() {
        let id: Vec<u8> = (0u8..16).collect();
        let signed_data = field_bytes(1, &id);
        let header = field_bytes(10000, &signed_data);
        let crx = crx_with_header(&header);
        assert_eq!(crx_id_bytes(&crx), Some(id));
    }

    #[test]
    fn crx3_坏输入一律_none() {
        assert_eq!(crx_id_bytes(b""), None);
        assert_eq!(crx_id_bytes(b"Cr24"), None);
        let mut v2 = b"Cr24".to_vec();
        v2.extend((2u32).to_le_bytes());
        v2.extend((0u32).to_le_bytes());
        assert_eq!(crx_id_bytes(&v2), None);
        let mut bad = b"Cr24".to_vec();
        bad.extend((3u32).to_le_bytes());
        bad.extend((999u32).to_le_bytes());
        bad.extend_from_slice(&[0u8; 8]);
        assert_eq!(crx_id_bytes(&bad), None);
        // header 里没有 10000 字段
        let crx = crx_with_header(&field_bytes(1, &[1, 2, 3]));
        assert_eq!(crx_id_bytes(&crx), None);
    }

    #[test]
    fn signed_data_只认长度_16_的字段_1() {
        let id: Vec<u8> = (0u8..16).collect();
        assert_eq!(signed_data_crx_id(&field_bytes(1, &id)), Some(id));
        assert_eq!(signed_data_crx_id(&field_bytes(1, &[0u8; 15])), None);
        assert_eq!(signed_data_crx_id(&field_bytes(2, &[0u8; 16])), None);
    }

    #[test]
    fn manifest_version_读取与回退() {
        let dir = std::env::temp_dir().join(format!(
            "tracker_manifest_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        assert_eq!(manifest_version(&dir), "1.0.0");
        std::fs::write(dir.join("manifest.json"), br#"{"version": "1.2.3"}"#).unwrap();
        assert_eq!(manifest_version(&dir), "1.2.3");
        std::fs::write(dir.join("manifest.json"), b"not json").unwrap();
        assert_eq!(manifest_version(&dir), "1.0.0");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
