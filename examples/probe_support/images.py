"""Small deterministic synthetic PNGs; no files, credentials or external URLs."""

import base64
import struct
import zlib


def color_png(rgb):
    """Create a 192x192 RGB PNG without an image-library dependency."""
    def chunk(kind, data):
        return (
            struct.pack(">I", len(data)) + kind + data
            + struct.pack(">I", zlib.crc32(kind + data) & 0xFFFFFFFF)
        )

    row = b"\x00" + bytes(rgb) * 192
    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", struct.pack(">IIBBBBB", 192, 192, 8, 2, 0, 0, 0))
        + chunk(b"IDAT", zlib.compress(row * 192))
        + chunk(b"IEND", b"")
    )
    return "data:image/png;base64," + base64.b64encode(png).decode("ascii")


def image_history(protocol):
    """Interleave two distinct images and text; answer depends on image order."""
    def text(value):
        return {"type": "text" if protocol == "chat" else "input_text", "text": value}

    def image(rgb):
        url = color_png(rgb)
        return (
            {"type": "image_url", "image_url": {"url": url}}
            if protocol == "chat"
            else {"type": "input_image", "image_url": url}
        )

    return [{"role": "user", "content": [
        text("Identify the dominant color of each image in order. Reply with exactly two lowercase English color names separated by a comma, no spaces or other text."),
        image((255, 0, 0)),
        text("Now the second image:"),
        image((0, 0, 255)),
    ]}]
