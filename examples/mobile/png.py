"""Independent, bounded PNG decoder for the RGB/RGBA proof artifacts."""
import struct
import zlib


def read_png(path):
    data = path.read_bytes()
    assert data[:8] == b"\x89PNG\r\n\x1a\n"
    offset, compressed, header = 8, bytearray(), None
    while offset < len(data):
        length, = struct.unpack_from(">I", data, offset)
        kind, body = data[offset + 4:offset + 8], data[offset + 8:offset + 8 + length]
        crc, = struct.unpack_from(">I", data, offset + 8 + length)
        assert zlib.crc32(kind + body) & 0xffffffff == crc
        offset += length + 12
        if kind == b"IHDR":
            header = struct.unpack(">IIBBBBB", body)
        elif kind == b"IDAT":
            compressed.extend(body)
        elif kind == b"IEND":
            break
    width, height, depth, color, compression, filter_method, interlace = header
    assert 0 < width <= 8192 and 0 < height <= 8192
    assert depth == 8 and color in (2, 6) and compression == filter_method == interlace == 0
    channels = 3 if color == 2 else 4
    stride = width * channels
    expected = (stride + 1) * height
    decoder = zlib.decompressobj()
    raw = decoder.decompress(compressed, expected + 1)
    assert len(raw) == expected and decoder.eof
    rows, previous = [], bytearray(stride)
    for y in range(height):
        start = y * (stride + 1)
        filter_type, row = raw[start], bytearray(raw[start + 1:start + 1 + stride])
        assert filter_type <= 4
        for x in range(stride):
            left = row[x - channels] if x >= channels else 0
            above = previous[x]
            diagonal = previous[x - channels] if x >= channels else 0
            if filter_type == 1:
                predictor = left
            elif filter_type == 2:
                predictor = above
            elif filter_type == 3:
                predictor = (left + above) // 2
            elif filter_type == 4:
                p = left + above - diagonal
                distances = (abs(p - left), abs(p - above), abs(p - diagonal))
                predictor = (left, above, diagonal)[distances.index(min(distances))]
            else:
                predictor = 0
            row[x] = (row[x] + predictor) & 255
        rows.append(bytes(row))
        previous = row
    return width, height, channels, rows
