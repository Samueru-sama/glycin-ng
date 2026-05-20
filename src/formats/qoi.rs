//! QOI decoder backed by the `qoi` crate.

use crate::{Error, Frame, Image, MemoryFormat, Result, Texture};

use super::DecodeOptions;

pub(crate) fn decode(bytes: &[u8], opts: &DecodeOptions) -> Result<Image> {
    let (header, pixels) = qoi::decode_to_vec(bytes).map_err(|e| match e {
        qoi::Error::IoError(io) => Error::Io(io),
        other => Error::Malformed(other.to_string()),
    })?;

    opts.limits
        .check_dimensions(header.width, header.height, 1)?;

    let format = match header.channels {
        qoi::Channels::Rgb => MemoryFormat::R8g8b8,
        qoi::Channels::Rgba => MemoryFormat::R8g8b8a8,
    };

    let bytes_per_pixel = format.bytes_per_pixel() as u64;
    let stride = bytes_per_pixel
        .checked_mul(header.width as u64)
        .filter(|s| *s <= u32::MAX as u64)
        .ok_or(Error::LimitExceeded("stride"))? as u32;

    let texture = Texture::from_parts(
        header.width,
        header.height,
        stride,
        format,
        pixels.into_boxed_slice(),
    )
    .ok_or_else(|| Error::Decoder {
        format: "qoi",
        message: "texture construction failed".into(),
    })?;

    let _ = opts.apply_transformations;
    Ok(Image::from_parts(
        "qoi",
        header.width,
        header.height,
        vec![Frame::new(texture, None)],
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Limits;

    fn opts() -> DecodeOptions {
        DecodeOptions {
            limits: Limits::default(),
            apply_transformations: true,
        }
    }

    fn make_qoi_rgba(width: u32, height: u32) -> Vec<u8> {
        let pixels = vec![0x40u8; (width * height * 4) as usize];
        qoi::encode_to_vec(&pixels, width, height).unwrap()
    }

    #[test]
    fn decodes_small_rgba() {
        let bytes = make_qoi_rgba(8, 8);
        let image = decode(&bytes, &opts()).unwrap();
        assert_eq!(image.width(), 8);
        assert_eq!(image.height(), 8);
        let texture = image.first_frame().unwrap().texture();
        assert_eq!(texture.format(), MemoryFormat::R8g8b8a8);
        assert_eq!(texture.data().len(), 8 * 8 * 4);
    }

    #[test]
    fn rejects_garbage() {
        let err = decode(b"qoifgarbage", &opts()).unwrap_err();
        assert!(matches!(err, Error::Malformed(_) | Error::Io(_)));
    }

    #[test]
    fn rejects_oversized() {
        let bytes = make_qoi_rgba(16, 16);
        let limits = Limits {
            max_width: 8,
            ..Limits::default()
        };
        let err = decode(
            &bytes,
            &DecodeOptions {
                limits,
                apply_transformations: true,
            },
        )
        .unwrap_err();
        assert!(matches!(err, Error::LimitExceeded("max_width")));
    }
}
