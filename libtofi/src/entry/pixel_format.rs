//! Convert between tiny-skia premultiplied RGBA8 and Wayland SHM ARGB8888.

/// Copy premultiplied RGBA8 pixels into an ARGB8888 buffer (little-endian byte order).
pub fn rgba_premul_to_argb8888(src: &[u8], dst: &mut [u8]) {
    debug_assert_eq!(src.len(), dst.len());
    for (s, d) in src.chunks_exact(4).zip(dst.chunks_exact_mut(4)) {
        let r = s[0];
        let g = s[1];
        let b = s[2];
        let a = s[3];
        d[0] = b;
        d[1] = g;
        d[2] = r;
        d[3] = a;
    }
}

/// Fill an RGBA8 premultiplied buffer with a solid color.
pub fn fill_rgba_premul(buf: &mut [u8], r: u8, g: u8, b: u8, a: u8) {
    let pr = ((r as u16 * a as u16 + 127) / 255) as u8;
    let pg = ((g as u16 * a as u16 + 127) / 255) as u8;
    let pb = ((b as u16 * a as u16 + 127) / 255) as u8;
    for px in buf.chunks_exact_mut(4) {
        px[0] = pr;
        px[1] = pg;
        px[2] = pb;
        px[3] = a;
    }
}
