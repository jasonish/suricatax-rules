// SPDX-License-Identifier: MIT

//! Read ZIP entries into memory only. Never apply archive paths, permissions,
//! or symlink targets to the filesystem.

use super::SourceFile;
use anyhow::{bail, ensure, Context, Result};
use rawzip::{CompressionMethod, ZipArchive, ZipFileHeaderRecord};
use std::io::Read;

pub(super) fn read(data: &[u8]) -> Result<Vec<SourceFile>> {
    let archive = ZipArchive::from_slice(data)?;
    let mut entries = archive.entries();
    let mut files = Vec::new();
    let mut count = 0u64;
    while let Some(entry) = entries.next_entry()? {
        count += 1;
        ensure!(
            !entry.flags().is_encrypted(),
            "Encrypted ZIP entries are not supported"
        );
        if entry.is_dir() {
            continue;
        }
        let filename = filename(&entry)?;
        let local = archive.get_entry(entry.wayfinder())?;
        ensure!(
            !local.local_header().flags().is_encrypted(),
            "Encrypted ZIP entries are not supported"
        );
        ensure!(
            local.local_header().compression_method() == entry.compression_method(),
            "ZIP compression method mismatch"
        );
        let decoder = decoder(
            local.data(),
            entry.compression_method(),
            entry.uncompressed_size_hint(),
        )?;
        let mut content = Vec::new();
        // Check both CRC32 and decompressed size, including stored entries.
        local
            .verifying_reader(decoder)
            .read_to_end(&mut content)
            .with_context(|| format!("Failed to read ZIP entry {filename}"))?;
        files.push(SourceFile { filename, content });
    }
    ensure!(count == archive.entries_hint(), "ZIP entry count mismatch");
    Ok(files)
}

fn filename(entry: &ZipFileHeaderRecord<'_>) -> Result<String> {
    let name = entry.file_path().as_bytes();
    if entry.flags().is_utf8() {
        return Ok(std::str::from_utf8(name)
            .context("Invalid ZIP entry filename")?
            .to_owned());
    }
    // Info-ZIP Unicode Path overrides the legacy name only when its CRC matches.
    for (id, data) in entry.extra_fields() {
        if id == rawzip::extra_fields::ExtraFieldId::INFO_ZIP_UNICODE_PATH
            && data.len() >= 5
            && data[0] == 1
            && u32::from_le_bytes(data[1..5].try_into()?) == rawzip::crc32(name)
        {
            return Ok(std::str::from_utf8(&data[5..])
                .context("Invalid ZIP Unicode filename")?
                .to_owned());
        }
    }
    Ok(name
        .iter()
        .map(|&b| char::from(oem_cp::Cp437::from(b)))
        .collect())
}

fn decoder(data: &[u8], method: CompressionMethod, size: u64) -> Result<Box<dyn Read + '_>> {
    Ok(match method {
        CompressionMethod::STORE => Box::new(data),
        CompressionMethod::DEFLATE => Box::new(flate2::read::DeflateDecoder::new(data)),
        CompressionMethod::DEFLATE64 => Box::new(deflate64::Deflate64Decoder::new(data)),
        CompressionMethod::BZIP2 => Box::new(bzip2::read::BzDecoder::new(data)),
        CompressionMethod::ZSTD | CompressionMethod::ZSTD_DEPRECATED => {
            Box::new(zstd::stream::read::Decoder::new(data)?)
        }
        CompressionMethod::XZ => Box::new(lzma_rust2::XzReader::new(data, false)),
        CompressionMethod::LZMA => {
            // ZIP LZMA has a 4-byte version/properties-length header followed by
            // 5 bytes of properties, not the standalone .lzma file header.
            ensure!(data.len() >= 9, "Truncated ZIP LZMA header");
            ensure!(
                u16::from_le_bytes([data[2], data[3]]) == 5,
                "Invalid ZIP LZMA properties length"
            );
            let dictionary_size = u32::from_le_bytes(data[5..9].try_into()?);
            Box::new(lzma_rust2::LzmaReader::new_with_props(
                &data[9..],
                size,
                data[4],
                dictionary_size,
                None,
            )?)
        }
        _ => bail!("Unsupported ZIP compression method: {method:?}"),
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use rawzip::path::EntryPath;
    use rawzip::{DataDescriptorOutput, ZipArchiveWriter};
    use std::io::Write;

    const RULE: &[u8] = b"alert ip any any -> any any (msg:\"zip test\"; sid:1; rev:1;)\n";
    const COMPRESSED: &[u8] = include_bytes!("../../tests/data/zip/compression.zip");

    fn stored_archive(name: EntryPath<'_>, encrypted: bool) -> Vec<u8> {
        let mut archive = ZipArchiveWriter::new(Vec::new());
        archive.new_dir("rules/").create().unwrap();
        let (mut entry, _) = archive.new_file(name).encrypted(encrypted).start().unwrap();
        entry.write_all(RULE).unwrap();
        entry
            .finish(DataDescriptorOutput::new(
                rawzip::crc32(RULE),
                RULE.len() as u64,
            ))
            .unwrap();
        archive.finish().unwrap()
    }

    #[test]
    fn reads_all_supported_compression_methods() {
        let files = read(COMPRESSED).unwrap();
        let names = [
            "stored",
            "deflate",
            "deflate64",
            "bzip2",
            "lzma",
            "xz",
            "zstd",
        ];
        assert_eq!(files.len(), names.len());
        for (file, name) in files.iter().zip(names) {
            assert_eq!(file.filename, format!("{name}.rules"));
            assert_eq!(file.content, RULE);
        }
    }

    #[test]
    fn skips_directories_and_preserves_utf8_names() {
        let data = stored_archive("rules/café.rules".into(), false);
        let files = read(&data).unwrap();
        assert_eq!(files.len(), 1);
        assert_eq!(files[0].filename, "rules/café.rules");
        assert_eq!(files[0].content, RULE);
    }

    #[test]
    fn decodes_legacy_cp437_names() {
        let data = stored_archive(EntryPath::verbatim(b"caf\x82.rules".as_slice()), false);
        assert_eq!(read(&data).unwrap()[0].filename, "café.rules");
    }

    #[test]
    fn honors_unicode_path_extra_field() {
        let mut archive = ZipArchiveWriter::new(Vec::new());
        let mut extra = vec![1];
        extra.extend(rawzip::crc32(b"legacy.rules").to_le_bytes());
        extra.extend("日本.rules".as_bytes());
        let (mut entry, _) = archive
            .new_file("legacy.rules")
            .extra_field(
                rawzip::extra_fields::ExtraFieldId::INFO_ZIP_UNICODE_PATH,
                &extra,
                rawzip::Header::LOCAL | rawzip::Header::CENTRAL,
            )
            .unwrap()
            .start()
            .unwrap();
        entry.write_all(RULE).unwrap();
        entry
            .finish(DataDescriptorOutput::new(
                rawzip::crc32(RULE),
                RULE.len() as u64,
            ))
            .unwrap();
        assert_eq!(
            read(&archive.finish().unwrap()).unwrap()[0].filename,
            "日本.rules"
        );
    }

    #[test]
    fn rejects_corrupt_crc_and_size() {
        let mut corrupt = COMPRESSED.to_vec();
        let offset = corrupt.windows(RULE.len()).position(|w| w == RULE).unwrap();
        corrupt[offset] ^= 1;
        assert!(read(&corrupt).is_err());

        let mut corrupt = COMPRESSED.to_vec();
        let offset = corrupt.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
        // The first central-directory record's uncompressed size.
        corrupt[offset + 24] += 1;
        assert!(read(&corrupt).is_err());
    }

    #[test]
    fn rejects_truncated_archives_and_wrong_entry_counts() {
        for len in [0, 4, 30, COMPRESSED.len() / 2, COMPRESSED.len() - 1] {
            assert!(read(&COMPRESSED[..len]).is_err(), "length {len}");
        }
        let mut corrupt = COMPRESSED.to_vec();
        let eocd = corrupt.len() - 22;
        corrupt[eocd + 8] += 1;
        corrupt[eocd + 10] += 1;
        assert!(read(&corrupt).is_err());
    }

    #[test]
    fn rejects_encryption_and_unsupported_compression() {
        assert!(read(&stored_archive("encrypted.rules".into(), true)).is_err());
        let mut corrupt = COMPRESSED.to_vec();
        let central = corrupt.windows(4).position(|w| w == b"PK\x01\x02").unwrap();
        corrupt[8..10].copy_from_slice(&99u16.to_le_bytes());
        corrupt[central + 10..central + 12].copy_from_slice(&99u16.to_le_bytes());
        assert!(read(&corrupt).is_err());
    }

    #[test]
    fn rejects_mismatched_compression_methods() {
        let mut corrupt = COMPRESSED.to_vec();
        corrupt[8..10].copy_from_slice(&8u16.to_le_bytes());
        assert!(read(&corrupt).is_err());
    }

    #[test]
    fn rejects_invalid_lzma_headers() {
        assert!(decoder(&[0; 8], CompressionMethod::LZMA, 0).is_err());
        assert!(decoder(&[0; 9], CompressionMethod::LZMA, 0).is_err());
    }
}
