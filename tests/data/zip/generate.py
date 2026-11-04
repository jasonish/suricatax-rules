#!/usr/bin/env python3
"""Generate compression.zip using Python's standard library and the zstd CLI.

The archive is deliberately generated independently of the Rust ZIP reader.
Deflate64 uses a stored block, whose encoding is shared with Deflate.
"""
import bz2
import io
import lzma
from pathlib import Path
import struct
import subprocess
import zipfile
import zlib

RULE = b'alert ip any any -> any any (msg:"zip test"; sid:1; rev:1;)\n'


def zip_lzma(data):
    # Let Python handle ZIP's LZMA-specific header and end marker.
    buffer = io.BytesIO()
    with zipfile.ZipFile(buffer, "w") as archive:
        archive.writestr("lzma.rules", data, compress_type=zipfile.ZIP_LZMA)
        info = archive.getinfo("lzma.rules")
    raw = buffer.getvalue()
    name_len, extra_len = struct.unpack_from("<HH", raw, 26)
    start = 30 + name_len + extra_len
    return raw[start:start + info.compress_size]


entries = [
    ("stored.rules", 0, RULE),
    ("deflate.rules", 8, zlib.compress(RULE, wbits=-15)),
    ("deflate64.rules", 9,
     b"\x01" + struct.pack("<HH", len(RULE), len(RULE) ^ 0xffff) + RULE),
    ("bzip2.rules", 12, bz2.compress(RULE)),
    ("lzma.rules", 14, zip_lzma(RULE)),
    ("xz.rules", 95, lzma.compress(RULE)),
    ("zstd.rules", 93,
     subprocess.run(["zstd", "-q", "-c"], input=RULE, capture_output=True,
                    check=True).stdout),
]

body = bytearray()
central = bytearray()
for name, method, compressed in entries:
    name = name.encode("ascii")
    offset = len(body)
    crc = zlib.crc32(RULE)
    flags = 2 if method == 14 else 0  # LZMA end-of-stream marker
    body += struct.pack("<IHHHHHIIIHH", 0x04034b50, 63, flags, method,
                        0, 0, crc, len(compressed), len(RULE), len(name), 0)
    body += name + compressed
    central += struct.pack("<IHHHHHHIIIHHHHHII", 0x02014b50, 63, 63, flags,
                           method, 0, 0, crc, len(compressed), len(RULE),
                           len(name), 0, 0, 0, 0, 0, offset)
    central += name
end = struct.pack("<IHHHHIIH", 0x06054b50, 0, 0, len(entries), len(entries),
                  len(central), len(body), 0)
Path(__file__).with_name("compression.zip").write_bytes(body + central + end)
