//! gdk-pixbuf loader module backed by `glycin-ng`.
//!
//! Build this crate to produce `libpixbufloader_glycin_ng.so`. Drop
//! the shared object into the gdk-pixbuf loaders directory (usually
//! `${libdir}/gdk-pixbuf-2.0/2.10.0/loaders/`) and run
//! `gdk-pixbuf-query-loaders --update-cache` so the framework
//! registers the new MIME types.
//!
//! The plugin exposes the two entry points gdk-pixbuf calls during
//! cache population: `fill_vtable` populates the function-pointer
//! table on the supplied [`GdkPixbufModule`], and `fill_info`
//! advertises the formats we can decode.
//!
//! Decoding goes through the same sandboxed worker thread that
//! powers [`glycin_ng::Loader`], so every call is landlocked and
//! seccomped (when both kernel features are available).

mod convert;
mod ffi;

use std::ffi::{CString, c_char, c_int, c_void};
use std::ptr;
use std::sync::OnceLock;

use glycin_ng::{Image, Loader};

use crate::convert::texture_to_rgba8;
use crate::ffi::{
    GDK_COLORSPACE_RGB, GDK_PIXBUF_FORMAT_THREADSAFE, GError, GdkPixbuf, GdkPixbufFormat,
    GdkPixbufModule, GdkPixbufModulePattern,
};

/// Populate the supplied module's vtable with our entry points.
///
/// Called once per loaded module by gdk-pixbuf during cache
/// population and at runtime when the framework opens this `.so`.
///
/// # Safety
///
/// `module` must point to a `GdkPixbufModule` allocated by
/// gdk-pixbuf; the layout must match the one we declare in
/// [`ffi::GdkPixbufModule`].
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fill_vtable(module: *mut GdkPixbufModule) {
    if module.is_null() {
        return;
    }
    unsafe {
        (*module).load = Some(load);
        (*module).load_xpm_data = None;
        (*module).begin_load = None;
        (*module).stop_load = None;
        (*module).load_increment = None;
        (*module).load_animation = None;
        (*module).save = None;
        (*module).save_to_callback = None;
        (*module).is_save_option_supported = None;
    }
}

/// Advertise the formats this loader handles.
///
/// gdk-pixbuf reads the populated [`GdkPixbufFormat`] when building
/// `loaders.cache`. Strings are statically allocated and must stay
/// valid for the program lifetime.
///
/// # Safety
///
/// `info` must point to a writable `GdkPixbufFormat`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn fill_info(info: *mut GdkPixbufFormat) {
    if info.is_null() {
        return;
    }
    let signatures = signatures();
    unsafe {
        (*info).name = c"glycin_ng".as_ptr();
        (*info).signature =
            signatures.as_ptr() as *mut GdkPixbufModulePattern;
        (*info).domain = c"glycin_ng".as_ptr();
        (*info).description = c"glycin-ng image loader".as_ptr();
        (*info).mime_types = mime_types().as_ptr() as *mut *const c_char;
        (*info).extensions = extensions().as_ptr() as *mut *const c_char;
        (*info).flags = GDK_PIXBUF_FORMAT_THREADSAFE;
        (*info).disabled = 0;
        (*info).license = c"MIT OR Apache-2.0".as_ptr();
    }
}

// Static signature byte strings. Each pattern matches gdk-pixbuf's
// `prefix_matches` rules: matching stops at the first NUL byte in
// `prefix` (unless a `mask` extends it), so embedded NULs cannot be
// used and must be skipped via a mask. The trailing `\0` on every
// slice is the C-string terminator gdk-pixbuf reads against.
struct SigDef {
    prefix: &'static [u8],
    mask: Option<&'static [u8]>,
    relevance: c_int,
}

const SIG_DEFS: &[SigDef] = &[
    SigDef {
        prefix: b"\x89PNG\r\n\x1a\n\0",
        mask: None,
        relevance: 100,
    },
    SigDef {
        prefix: b"\xff\xd8\xff\0",
        mask: None,
        relevance: 100,
    },
    SigDef {
        prefix: b"GIF87a\0",
        mask: None,
        relevance: 100,
    },
    SigDef {
        prefix: b"GIF89a\0",
        mask: None,
        relevance: 100,
    },
    SigDef {
        prefix: b"RIFF    WEBP\0",
        mask: Some(b"xxxx    xxxx\0"),
        relevance: 100,
    },
    SigDef {
        prefix: b"II*\0",
        mask: None,
        relevance: 80,
    },
    SigDef {
        prefix: b"MM\0",
        mask: None,
        relevance: 80,
    },
    SigDef {
        prefix: b"BM\0",
        mask: None,
        relevance: 60,
    },
    SigDef {
        prefix: b"qoif\0",
        mask: None,
        relevance: 100,
    },
    SigDef {
        prefix: b"\xff\x0a\0",
        mask: None,
        relevance: 100,
    },
    SigDef {
        prefix: b"DDS \0",
        mask: None,
        relevance: 100,
    },
    SigDef {
        prefix: b"\x76\x2f\x31\x01\0",
        mask: None,
        relevance: 100,
    },
];

struct Patterns(Vec<GdkPixbufModulePattern>);
struct PointerList(Vec<*const c_char>);

// SAFETY: both wrappers hold pointers that target `'static` byte
// slices compiled into the binary. The vectors are populated once
// inside `get_or_init` and never mutated again. No interior
// mutability means concurrent reads through the shared `&'static`
// reference are sound.
unsafe impl Sync for Patterns {}
unsafe impl Send for Patterns {}
unsafe impl Sync for PointerList {}
unsafe impl Send for PointerList {}

fn signatures() -> &'static [GdkPixbufModulePattern] {
    static CELL: OnceLock<Patterns> = OnceLock::new();
    let inner = CELL.get_or_init(|| {
        let mut v: Vec<GdkPixbufModulePattern> = SIG_DEFS
            .iter()
            .map(|d| GdkPixbufModulePattern {
                prefix: d.prefix.as_ptr() as *const c_char,
                mask: d
                    .mask
                    .map(|m| m.as_ptr() as *const c_char)
                    .unwrap_or(ptr::null()),
                relevance: d.relevance,
            })
            .collect();
        v.push(GdkPixbufModulePattern {
            prefix: ptr::null(),
            mask: ptr::null(),
            relevance: 0,
        });
        Patterns(v)
    });
    &inner.0
}

fn mime_types() -> &'static [*const c_char] {
    static CELL: OnceLock<PointerList> = OnceLock::new();
    let inner = CELL.get_or_init(|| {
        const NAMES: &[&[u8]] = &[
            b"image/png\0",
            b"image/apng\0",
            b"image/jpeg\0",
            b"image/gif\0",
            b"image/webp\0",
            b"image/tiff\0",
            b"image/bmp\0",
            b"image/x-bmp\0",
            b"image/x-ico\0",
            b"image/x-icon\0",
            b"image/vnd.microsoft.icon\0",
            b"image/x-tga\0",
            b"image/x-targa\0",
            b"image/x-qoi\0",
            b"image/x-exr\0",
            b"image/x-portable-anymap\0",
            b"image/x-portable-bitmap\0",
            b"image/x-portable-graymap\0",
            b"image/x-portable-pixmap\0",
            b"image/vnd-ms.dds\0",
            b"image/jxl\0",
        ];
        let mut v: Vec<*const c_char> =
            NAMES.iter().map(|n| n.as_ptr() as *const c_char).collect();
        v.push(ptr::null());
        PointerList(v)
    });
    &inner.0
}

fn extensions() -> &'static [*const c_char] {
    static CELL: OnceLock<PointerList> = OnceLock::new();
    let inner = CELL.get_or_init(|| {
        const EXTS: &[&[u8]] = &[
            b"png\0",
            b"apng\0",
            b"jpg\0",
            b"jpeg\0",
            b"jpe\0",
            b"jfif\0",
            b"gif\0",
            b"webp\0",
            b"tif\0",
            b"tiff\0",
            b"bmp\0",
            b"dib\0",
            b"ico\0",
            b"cur\0",
            b"tga\0",
            b"qoi\0",
            b"exr\0",
            b"pbm\0",
            b"pgm\0",
            b"ppm\0",
            b"pnm\0",
            b"pam\0",
            b"dds\0",
            b"jxl\0",
        ];
        let mut v: Vec<*const c_char> =
            EXTS.iter().map(|e| e.as_ptr() as *const c_char).collect();
        v.push(ptr::null());
        PointerList(v)
    });
    &inner.0
}

/// `GdkPixbufModuleLoadFunc`: read the entire image from a `FILE*`
/// stream, decode through glycin-ng, and hand back a freshly
/// allocated `GdkPixbuf`.
unsafe extern "C" fn load(
    file: *mut libc::FILE,
    error: *mut *mut GError,
) -> *mut GdkPixbuf {
    let bytes = match unsafe { read_file_to_vec(file) } {
        Ok(b) => b,
        Err(msg) => {
            unsafe { set_error(error, &msg) };
            return ptr::null_mut();
        }
    };

    let image = match Loader::new_bytes(bytes).load() {
        Ok(img) => img,
        Err(e) => {
            unsafe { set_error(error, &e.to_string()) };
            return ptr::null_mut();
        }
    };

    unsafe { image_to_pixbuf(&image, error) }
}

unsafe fn read_file_to_vec(file: *mut libc::FILE) -> Result<Vec<u8>, String> {
    if file.is_null() {
        return Err("null FILE*".into());
    }
    let mut buf = Vec::new();
    let mut chunk = [0u8; 65536];
    loop {
        let n = unsafe {
            libc::fread(
                chunk.as_mut_ptr() as *mut c_void,
                1,
                chunk.len(),
                file,
            )
        };
        if n == 0 {
            if unsafe { libc::ferror(file) } != 0 {
                return Err("fread reported an error".into());
            }
            break;
        }
        buf.extend_from_slice(&chunk[..n]);
        if n < chunk.len() {
            break;
        }
    }
    Ok(buf)
}

unsafe fn image_to_pixbuf(image: &Image, error: *mut *mut GError) -> *mut GdkPixbuf {
    let Some(frame) = image.first_frame() else {
        unsafe { set_error(error, "image contained no frames") };
        return ptr::null_mut();
    };
    let texture = frame.texture();
    let width = texture.width() as c_int;
    let height = texture.height() as c_int;
    let (rgba, rowstride) = texture_to_rgba8(texture);

    let gbytes = unsafe {
        crate::ffi::g_bytes_new(rgba.as_ptr() as *const c_void, rgba.len())
    };
    if gbytes.is_null() {
        unsafe { set_error(error, "g_bytes_new returned NULL") };
        return ptr::null_mut();
    }

    let pixbuf = unsafe {
        crate::ffi::gdk_pixbuf_new_from_bytes(
            gbytes,
            GDK_COLORSPACE_RGB,
            1,
            8,
            width,
            height,
            rowstride as c_int,
        )
    };

    unsafe { crate::ffi::g_bytes_unref(gbytes) };

    if pixbuf.is_null() {
        unsafe { set_error(error, "gdk_pixbuf_new_from_bytes returned NULL") };
    }
    pixbuf
}

unsafe fn set_error(error: *mut *mut GError, msg: &str) {
    if error.is_null() {
        return;
    }
    let cmsg = match CString::new(msg) {
        Ok(c) => c,
        Err(_) => CString::new("error").unwrap(),
    };
    let domain = unsafe {
        crate::ffi::g_quark_from_static_string(c"glycin_ng".as_ptr())
    };
    unsafe {
        crate::ffi::g_set_error_literal(error, domain, 0, cmsg.as_ptr());
    }
}
