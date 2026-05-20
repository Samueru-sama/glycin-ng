//! Pixel-level orientation transforms.
//!
//! Each transform operates on the raw byte buffer of a
//! [`Texture`](crate::Texture). The byte size of a pixel is
//! determined by the [`MemoryFormat`](crate::MemoryFormat); rows are
//! packed at `width * bytes_per_pixel` (the stride is rewritten by
//! the output texture).

use crate::{Frame, Image, Orientation, Texture};

/// Apply `orientation` to every frame in `image`, replacing each
/// frame's texture with the transformed pixels.
///
/// Updates the image's reported `width` and `height` when the
/// orientation swaps axes.
pub(crate) fn bake_into_frames(image: &mut Image, orientation: Orientation) {
    if orientation == Orientation::Normal {
        return;
    }
    let new_frames: Vec<Frame> = image
        .frames()
        .iter()
        .map(|f| {
            let new_tex = transform_texture(f.texture(), orientation);
            Frame::new(new_tex, f.delay())
        })
        .collect();
    let new_w = if orientation.swaps_axes() {
        image.height()
    } else {
        image.width()
    };
    let new_h = if orientation.swaps_axes() {
        image.width()
    } else {
        image.height()
    };
    image.replace_frames(new_frames, new_w, new_h);
}

fn transform_texture(texture: &Texture, orientation: Orientation) -> Texture {
    let format = texture.format();
    let bpp = format.bytes_per_pixel() as usize;
    let width = texture.width() as usize;
    let height = texture.height() as usize;
    let src = texture.data();
    let src_stride = texture.stride() as usize;

    let (out_w, out_h) = if orientation.swaps_axes() {
        (height, width)
    } else {
        (width, height)
    };
    let out_stride = out_w * bpp;
    let mut dst = vec![0u8; out_stride * out_h];

    for sy in 0..height {
        for sx in 0..width {
            let (dx, dy) = mapped_coord(orientation, sx, sy, width, height);
            let src_off = sy * src_stride + sx * bpp;
            let dst_off = dy * out_stride + dx * bpp;
            dst[dst_off..dst_off + bpp].copy_from_slice(&src[src_off..src_off + bpp]);
        }
    }

    Texture::from_parts(
        out_w as u32,
        out_h as u32,
        out_stride as u32,
        format,
        dst.into_boxed_slice(),
    )
    .expect("byte-aligned transform always produces a valid texture")
}

fn mapped_coord(o: Orientation, x: usize, y: usize, w: usize, h: usize) -> (usize, usize) {
    match o {
        Orientation::Normal => (x, y),
        Orientation::FlipHorizontal => (w - 1 - x, y),
        Orientation::Rotate180 => (w - 1 - x, h - 1 - y),
        Orientation::FlipVertical => (x, h - 1 - y),
        Orientation::Transpose => (y, x),
        Orientation::Rotate90 => (h - 1 - y, x),
        Orientation::Transverse => (h - 1 - y, w - 1 - x),
        Orientation::Rotate270 => (y, w - 1 - x),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn checker_2x3() -> Texture {
        let data: Vec<u8> = vec![
            10, 20, // y=0
            30, 40, // y=1
            50, 60, // y=2
        ];
        Texture::from_parts(2, 3, 2, crate::MemoryFormat::G8, data.into_boxed_slice()).unwrap()
    }

    #[test]
    fn rotate_180_swaps_corners() {
        let t = checker_2x3();
        let out = transform_texture(&t, Orientation::Rotate180);
        assert_eq!(out.width(), 2);
        assert_eq!(out.height(), 3);
        // Original: [[10,20],[30,40],[50,60]] -> rotated 180:
        // [[60,50],[40,30],[20,10]]
        assert_eq!(out.data(), &[60, 50, 40, 30, 20, 10]);
    }

    #[test]
    fn rotate_90_swaps_axes() {
        let t = checker_2x3();
        let out = transform_texture(&t, Orientation::Rotate90);
        assert_eq!(out.width(), 3);
        assert_eq!(out.height(), 2);
        // 90 CW: column 0 becomes row 0 reversed.
        // Original cols: [10,30,50] and [20,40,60].
        // After 90 CW: row 0 = [50,30,10], row 1 = [60,40,20].
        assert_eq!(out.data(), &[50, 30, 10, 60, 40, 20]);
    }

    #[test]
    fn flip_horizontal_mirrors_rows() {
        let t = checker_2x3();
        let out = transform_texture(&t, Orientation::FlipHorizontal);
        assert_eq!(out.width(), 2);
        assert_eq!(out.height(), 3);
        assert_eq!(out.data(), &[20, 10, 40, 30, 60, 50]);
    }

    #[test]
    fn flip_vertical_swaps_rows() {
        let t = checker_2x3();
        let out = transform_texture(&t, Orientation::FlipVertical);
        assert_eq!(out.data(), &[50, 60, 30, 40, 10, 20]);
    }

    #[test]
    fn rotate_270_inverts_90() {
        let t = checker_2x3();
        let r90 = transform_texture(&t, Orientation::Rotate90);
        let back = transform_texture(&r90, Orientation::Rotate270);
        assert_eq!(back.data(), t.data());
        assert_eq!(back.width(), t.width());
        assert_eq!(back.height(), t.height());
    }

    #[test]
    fn transpose_round_trips() {
        let t = checker_2x3();
        let trans = transform_texture(&t, Orientation::Transpose);
        let back = transform_texture(&trans, Orientation::Transpose);
        assert_eq!(back.data(), t.data());
    }
}
