# glycin-ng-libglycin-shim

ABI-compatible `libglycin-2.so.0` that forwards every `gly_*` call to
`glycin_ng`. Drop-in replacement for upstream `libglycin` on systems
that have it as a hard `NEEDED` dependency.

## When you want this

Newer `gdk-pixbuf` (2.42.10+) ships with optional glycin support
compiled directly into `libgdk_pixbuf-2.0.so.0`. Arch's gdk-pixbuf2
package turns it on, so its `libgdk_pixbuf-2.0.so.0` carries a hard
`NEEDED libglycin-2.so.0`. Without the shim you either:

- Keep upstream `libglycin` and pull in `bwrap` plus four per-format
  loader binaries (`glycin-image-rs`, `glycin-svg`, `glycin-jxl`,
  `glycin-heif`) plus the LGPL/MPL libraries those link against
  (`librsvg`, `libjxl`, `libheif`, `libopenraw`, ...), or
- Don't ship gdk-pixbuf at all and find another image stack.

With this shim, `libgdk_pixbuf-2.0.so.0` links cleanly, every decode
happens in-process inside `glycin_ng`'s sandbox, and the helpers can
be removed.

## Size comparison

Both `libglycin-2.so.0` libraries are roughly the same size on disk;
the difference is what each one pulls into the install. Numbers below
are from `pacman -Q` on Arch x86_64 with the `glycin` and
`glycin-loaders` packages installed and our shim built with release
profile defaults:

| Component               | Upstream libglycin                                                                   | This shim          |
|-------------------------|--------------------------------------------------------------------------------------|--------------------|
| `libglycin-2.so.0`      | ~3.2 MB                                                                              | ~3.3 MB            |
| Sandbox helper          | `bwrap` (~80 KB)                                                                     | (in-process)       |
| Per-format loaders      | `glycin-image-rs` (~5.9 MB), `glycin-svg` (~2.1 MB), `glycin-jxl` (~2.5 MB), `glycin-heif` (~2.6 MB) | (in-process)       |
| Transitive libraries    | `librsvg`, `libjxl`, `libjxl_threads`, `libjxl_cms`, `libdav1d`, `libheif`, ...      | none added         |
| **Install weight**      | **~25-30 MB**                                                                        | **~3.3 MB**        |

The shim is one self-contained shared library that bundles every
default decoder. Upstream is a thin client of separately-installed
helper binaries that link copyleft codec libraries.

## Build

```
cargo build --release -p glycin-ng-libglycin-shim
```

Output: `target/release/libglycin_2.so` (~3.3 MB stripped on
x86_64-gnu, with every default decoder bundled). `build.rs` pins the
SONAME to `libglycin-2.so.0` regardless of the filename, so it
resolves any caller's `NEEDED libglycin-2.so.0` cleanly.

## Install (drop-in)

```
LIBDIR=$(pkg-config --variable=libdir gdk-pixbuf-2.0)
sudo install -Dm755 target/release/libglycin_2.so \
    "$LIBDIR/libglycin-2.so.0"
sudo ln -sf libglycin-2.so.0 "$LIBDIR/libglycin-2.so"
```

Verify the swap took:

```
ldd "$(pkg-config --variable=libdir gdk-pixbuf-2.0)/libgdk_pixbuf-2.0.so.0" \
    | grep libglycin
```

The output should point at the file you just installed. The original
`${libdir}/glycin-loaders/` directory and the `bwrap` binary are
unused once the shim is in place and can be removed.

## Symbol coverage

### Load path

| Symbol                                                                 | Behavior                                                          |
|------------------------------------------------------------------------|-------------------------------------------------------------------|
| `gly_loader_new`                                                       | path-backed `GFile` (non-native files return `NULL`)              |
| `gly_loader_new_for_bytes`                                             | in-memory buffer                                                  |
| `gly_loader_new_for_stream`                                            | whole stream read into memory before decode                       |
| `gly_loader_set_sandbox_selector`                                      | accepted, ignored (see below)                                     |
| `gly_loader_set_apply_transformations`                                 | EXIF orientation toggle                                           |
| `gly_loader_set_accepted_memory_formats`                               | honored; output converted to a format in the set                  |
| `gly_loader_load`                                                      | decode                                                            |
| `gly_image_get_width` / `_height` / `_transformation_orientation`      |                                                                   |
| `gly_image_get_specific_frame`                                         | advances a cursor through animation frames                        |
| `gly_frame_request_new` / `_set_loop_animation` / `_set_scale`         | stored; scaling is not yet applied                                |
| `gly_frame_get_width` / `_height` / `_stride` / `_memory_format` / `_buf_bytes` / `_delay` |                                                       |
| `gly_memory_format_has_alpha` / `_is_premultiplied`                    |                                                                   |
| `gly_loader_error_quark`                                               |                                                                   |

### Encode path

All `gly_creator_*` and `gly_encoded_image_*` symbols exist for ABI
compatibility but return `NULL` / `FALSE`. Callers that check return
values fall through to their own encoder.

### Metadata

`gly_image_get_metadata_keys` and `gly_image_get_metadata_key_value`
return `NULL`. EXIF and ICC blobs are available on the underlying
`glycin_ng::Image` but not yet projected through the `gly_*`
key/value surface.

## Sandbox model

Every decode goes through `glycin_ng::Loader::new_bytes(...).load()`,
which spawns a `glycin-ng-worker` thread and applies landlock plus
seccomp on it before invoking the decoder. The host process stays
unrestricted.

`gly_loader_set_sandbox_selector` is **intentionally ignored**.
Upstream supports a `GLY_SANDBOX_SELECTOR_NOT_SANDBOXED` value that
turns off the bwrap container; in upstream that's necessary because
bwrap fails to nest inside Flatpak / AppImage. We don't need bwrap to
nest (landlock and seccomp are per-thread `prctl` calls that always
work), so honoring the selector would only let `LD_PRELOAD` shims
(such as AnyLinux's `gtk-class-fix.so`) disable the in-process
sandbox post-load. The setter is kept for ABI compatibility but
performs no state change.

## Object lifetimes

`GlyLoader`, `GlyImage`, `GlyFrame`, and `GlyFrameRequest` are
base-class `GObject`s with Rust state attached via
`g_object_set_data_full`. State is freed by the attached destroy
notify when the host calls `g_object_unref` and the refcount hits
zero. We skip full `GType` registration; consumers that introspect
via `G_TYPE_CHECK_INSTANCE_*` will not see a `GLY_TYPE_LOADER`
derivation. Every gdk-pixbuf code path that uses these handles
strictly through the `gly_*` C surface is unaffected.

## License

MIT OR Apache-2.0.
