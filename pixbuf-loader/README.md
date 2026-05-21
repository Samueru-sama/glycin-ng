# glycin-ng-pixbuf-loader

A single gdk-pixbuf loader module that handles every supported image
MIME by routing decode through `glycin_ng`. Replaces the per-format
`libpixbufloader-png.so`, `libpixbufloader-jpeg.so`, ... pile with
one ~5 MB `.so`, each decode sandboxed on a worker thread.

## When you want this

If your distro ships gdk-pixbuf with per-format loader modules, you
can swap them for this one and get:

- A single loader entry instead of ~15
- Landlock + seccomp on every gdk-pixbuf decode (the existing
  per-format loaders run unsandboxed in the calling process)
- The ability to drop upstream glycin entirely if your stack also
  uses [`glycin-ng-libglycin-shim`](../libglycin-shim/)

Packaging an AppImage or Flatpak? Install this loader exclusively in
the bundle, never ship `libpixbufloader-*` or `libglycin`, and
gdk-pixbuf has one loader to find for everything.

## Build

```
cargo build --release -p glycin-ng-pixbuf-loader
```

Output: `target/release/libpixbufloader_glycin_ng.so` (~5 MB
stripped, with every codec `glycin_ng` enables by default).

## Install (system)

```
LOADERDIR=$(pkg-config --variable=gdk_pixbuf_moduledir gdk-pixbuf-2.0)
sudo install -Dm755 target/release/libpixbufloader_glycin_ng.so \
    "$LOADERDIR/libpixbufloader_glycin_ng.so"
sudo gdk-pixbuf-query-loaders --update-cache
```

Verify it registered:

```
gdk-pixbuf-query-loaders | grep -A1 glycin
```

You should see one entry advertising every supported MIME type and
extension. If you also want to remove the upstream per-format
loaders, delete them from `$LOADERDIR` before re-running
`gdk-pixbuf-query-loaders --update-cache`.

## Install (bundled, non-system)

For AppImage / Flatpak / container packages where you control the
gdk-pixbuf moduledir:

```
GDK_PIXBUF_MODULEDIR=/path/to/loaders \
    gdk-pixbuf-query-loaders > /path/to/loaders.cache
```

Then run the consumer with:

```
GDK_PIXBUF_MODULE_FILE=/path/to/loaders.cache your-app
```

## Supported MIME types

PNG (incl. APNG), JPEG, GIF (animated), WebP (animated), TIFF, BMP,
ICO / CUR, TGA, QOI, OpenEXR, PNM family, DDS, JPEG XL, SVG
(including GTK symbolic-icon wrappers with `xi:include` data URIs).

## Sandbox

Every decode goes through `glycin_ng::Loader::new_bytes(...).load()`,
which spawns a worker thread, applies landlock + seccomp on it, runs
the decoder, joins, and returns the bytes. The calling
gdk-pixbuf thread and the host process stay unrestricted.

See the parent [`glycin-ng` README](../README.md) for the full
sandbox model.

## Known limits

- **Streaming**. `begin_load` / `load_increment` / `stop_load` are
  wired, but the worker accumulates the full buffer before decoding,
  so the `prepared` and `updated` callbacks fire once at
  end-of-stream rather than per row. Per-row progressive previews
  would require a streaming decoder behind every format.
- **Animations**. Routed through `GdkPixbufSimpleAnim`, which holds
  a single frame rate. Animations with non-uniform per-frame delays
  are flattened to the mean. Per-frame timing requires a custom
  `GdkPixbufAnimation` subclass and is not yet implemented.
- **Bit depth**. Output is always 8-bit RGBA. This is inherent to
  gdk-pixbuf (`gdk_pixbuf_get_bits_per_sample` always returns 8).
  HDR / wider-than-8-bit consumers should drive `GdkTexture`
  directly via a future `glycin-ng-gtk4` adapter.

## License

MIT OR Apache-2.0.
