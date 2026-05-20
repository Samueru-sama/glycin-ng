//! Convert any [`glycin_ng::Texture`] to a tightly-packed RGBA8
//! buffer suitable for `gdk_pixbuf_new_from_bytes`.
//!
//! gdk-pixbuf only ships RGB and RGBA at 8 bits per channel. Every
//! other format we produce is reduced to RGBA8 here: 16-bit channels
//! are right-shifted by 8, half-float channels are skipped (treated
//! as black for now), 32-bit floats are clamped to `[0, 1]` and
//! scaled, premultiplied colors are unpremultiplied, and grayscale
//! is replicated across R, G, B.

use glycin_ng::{MemoryFormat, Texture};

/// Returns `(rgba_bytes, rowstride)`. `rowstride == width * 4`.
pub(crate) fn texture_to_rgba8(texture: &Texture) -> (Vec<u8>, u32) {
    let width = texture.width() as usize;
    let height = texture.height() as usize;
    let src_stride = texture.stride() as usize;
    let bpp = texture.format().bytes_per_pixel() as usize;
    let src = texture.data();
    let mut out = Vec::with_capacity(width * height * 4);

    for y in 0..height {
        let row_start = y * src_stride;
        for x in 0..width {
            let p = &src[row_start + x * bpp..row_start + x * bpp + bpp];
            let (r, g, b, a) = sample_rgba8(texture.format(), p);
            out.extend_from_slice(&[r, g, b, a]);
        }
    }

    (out, (width * 4) as u32)
}

fn sample_rgba8(format: MemoryFormat, p: &[u8]) -> (u8, u8, u8, u8) {
    match format {
        MemoryFormat::G8 => (p[0], p[0], p[0], 255),
        MemoryFormat::G8a8 => (p[0], p[0], p[0], p[1]),
        MemoryFormat::G8a8Premultiplied => {
            let (g, a) = unpremul_g8(p[0], p[1]);
            (g, g, g, a)
        }
        MemoryFormat::G16 => {
            let v = u16::from_ne_bytes([p[0], p[1]]);
            let v8 = (v >> 8) as u8;
            (v8, v8, v8, 255)
        }
        MemoryFormat::G16a16 => {
            let v = (u16::from_ne_bytes([p[0], p[1]]) >> 8) as u8;
            let a = (u16::from_ne_bytes([p[2], p[3]]) >> 8) as u8;
            (v, v, v, a)
        }
        MemoryFormat::G16a16Premultiplied => {
            let v = (u16::from_ne_bytes([p[0], p[1]]) >> 8) as u8;
            let a = (u16::from_ne_bytes([p[2], p[3]]) >> 8) as u8;
            let (g, a) = unpremul_g8(v, a);
            (g, g, g, a)
        }
        MemoryFormat::R8g8b8 => (p[0], p[1], p[2], 255),
        MemoryFormat::R8g8b8a8 => (p[0], p[1], p[2], p[3]),
        MemoryFormat::R8g8b8a8Premultiplied => unpremul_rgb8(p[0], p[1], p[2], p[3]),
        MemoryFormat::B8g8r8 => (p[2], p[1], p[0], 255),
        MemoryFormat::B8g8r8a8 => (p[2], p[1], p[0], p[3]),
        MemoryFormat::B8g8r8a8Premultiplied => unpremul_rgb8(p[2], p[1], p[0], p[3]),
        MemoryFormat::A8r8g8b8 => (p[1], p[2], p[3], p[0]),
        MemoryFormat::A8r8g8b8Premultiplied => unpremul_rgb8(p[1], p[2], p[3], p[0]),
        MemoryFormat::A8b8g8r8 => (p[3], p[2], p[1], p[0]),
        MemoryFormat::R16g16b16 => {
            let r = (u16::from_ne_bytes([p[0], p[1]]) >> 8) as u8;
            let g = (u16::from_ne_bytes([p[2], p[3]]) >> 8) as u8;
            let b = (u16::from_ne_bytes([p[4], p[5]]) >> 8) as u8;
            (r, g, b, 255)
        }
        MemoryFormat::R16g16b16a16 => {
            let r = (u16::from_ne_bytes([p[0], p[1]]) >> 8) as u8;
            let g = (u16::from_ne_bytes([p[2], p[3]]) >> 8) as u8;
            let b = (u16::from_ne_bytes([p[4], p[5]]) >> 8) as u8;
            let a = (u16::from_ne_bytes([p[6], p[7]]) >> 8) as u8;
            (r, g, b, a)
        }
        MemoryFormat::R16g16b16a16Premultiplied => {
            let r = (u16::from_ne_bytes([p[0], p[1]]) >> 8) as u8;
            let g = (u16::from_ne_bytes([p[2], p[3]]) >> 8) as u8;
            let b = (u16::from_ne_bytes([p[4], p[5]]) >> 8) as u8;
            let a = (u16::from_ne_bytes([p[6], p[7]]) >> 8) as u8;
            unpremul_rgb8(r, g, b, a)
        }
        MemoryFormat::R16g16b16Float | MemoryFormat::R16g16b16a16Float => {
            (0, 0, 0, 255)
        }
        MemoryFormat::R32g32b32Float => {
            let r = float_to_u8(f32::from_ne_bytes([p[0], p[1], p[2], p[3]]));
            let g = float_to_u8(f32::from_ne_bytes([p[4], p[5], p[6], p[7]]));
            let b = float_to_u8(f32::from_ne_bytes([p[8], p[9], p[10], p[11]]));
            (r, g, b, 255)
        }
        MemoryFormat::R32g32b32a32Float => {
            let r = float_to_u8(f32::from_ne_bytes([p[0], p[1], p[2], p[3]]));
            let g = float_to_u8(f32::from_ne_bytes([p[4], p[5], p[6], p[7]]));
            let b = float_to_u8(f32::from_ne_bytes([p[8], p[9], p[10], p[11]]));
            let a = float_to_u8(f32::from_ne_bytes([p[12], p[13], p[14], p[15]]));
            (r, g, b, a)
        }
        MemoryFormat::R32g32b32a32FloatPremultiplied => {
            let r = float_to_u8(f32::from_ne_bytes([p[0], p[1], p[2], p[3]]));
            let g = float_to_u8(f32::from_ne_bytes([p[4], p[5], p[6], p[7]]));
            let b = float_to_u8(f32::from_ne_bytes([p[8], p[9], p[10], p[11]]));
            let a = float_to_u8(f32::from_ne_bytes([p[12], p[13], p[14], p[15]]));
            unpremul_rgb8(r, g, b, a)
        }
        _ => (0, 0, 0, 255),
    }
}

fn unpremul_g8(g: u8, a: u8) -> (u8, u8) {
    if a == 0 {
        return (0, 0);
    }
    let g = ((g as u32 * 255) / a as u32).min(255) as u8;
    (g, a)
}

fn unpremul_rgb8(r: u8, g: u8, b: u8, a: u8) -> (u8, u8, u8, u8) {
    if a == 0 {
        return (0, 0, 0, 0);
    }
    let unp = |c: u8| ((c as u32 * 255) / a as u32).min(255) as u8;
    (unp(r), unp(g), unp(b), a)
}

fn float_to_u8(f: f32) -> u8 {
    let clamped = f.clamp(0.0, 1.0);
    (clamped * 255.0 + 0.5) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tex(format: MemoryFormat, data: Vec<u8>, w: u32, h: u32) -> Texture {
        let stride = w * format.bytes_per_pixel() as u32;
        Texture::from_parts(w, h, stride, format, data.into_boxed_slice()).unwrap()
    }

    #[test]
    fn rgba8_passes_through() {
        let t = tex(
            MemoryFormat::R8g8b8a8,
            vec![1, 2, 3, 4, 5, 6, 7, 8],
            2,
            1,
        );
        let (bytes, stride) = texture_to_rgba8(&t);
        assert_eq!(stride, 8);
        assert_eq!(bytes, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn rgb8_gains_opaque_alpha() {
        let t = tex(MemoryFormat::R8g8b8, vec![1, 2, 3, 4, 5, 6], 2, 1);
        let (bytes, _) = texture_to_rgba8(&t);
        assert_eq!(bytes, vec![1, 2, 3, 255, 4, 5, 6, 255]);
    }

    #[test]
    fn bgr_channels_get_swapped() {
        let t = tex(MemoryFormat::B8g8r8, vec![10, 20, 30], 1, 1);
        let (bytes, _) = texture_to_rgba8(&t);
        assert_eq!(bytes, vec![30, 20, 10, 255]);
    }

    #[test]
    fn g8_replicates_across_channels() {
        let t = tex(MemoryFormat::G8, vec![42, 200], 2, 1);
        let (bytes, _) = texture_to_rgba8(&t);
        assert_eq!(bytes, vec![42, 42, 42, 255, 200, 200, 200, 255]);
    }

    #[test]
    fn g8a8_premultiplied_is_unpremultiplied() {
        // Premultiplied (g=50, a=100) -> straight (g=128 ish, a=100).
        let t = tex(MemoryFormat::G8a8Premultiplied, vec![50, 100], 1, 1);
        let (bytes, _) = texture_to_rgba8(&t);
        // 50 / 100 * 255 = 127.5 -> 127
        assert_eq!(bytes[0], 127);
        assert_eq!(bytes[3], 100);
    }

    #[test]
    fn g16_downsamples() {
        let v: u16 = 0xABCD;
        let bytes = v.to_ne_bytes();
        let t = tex(MemoryFormat::G16, bytes.to_vec(), 1, 1);
        let (out, _) = texture_to_rgba8(&t);
        assert_eq!(out[0], 0xAB);
    }

    #[test]
    fn f32_clamps_and_scales() {
        let mut data = Vec::new();
        data.extend_from_slice(&0.0_f32.to_ne_bytes());
        data.extend_from_slice(&0.5_f32.to_ne_bytes());
        data.extend_from_slice(&1.5_f32.to_ne_bytes()); // clamps to 1.0
        let t = tex(MemoryFormat::R32g32b32Float, data, 1, 1);
        let (out, _) = texture_to_rgba8(&t);
        assert_eq!(out[0], 0);
        assert_eq!(out[1], 128); // 0.5 * 255 + 0.5 = 128
        assert_eq!(out[2], 255);
    }
}
