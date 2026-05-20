/* glycin-ng C ABI header.
 *
 * Link against libglycin_ng.so built with `cargo build --release
 * --features c-api`. All functions are thread-compatible but not
 * thread-safe per handle; do not share a single GlycinNgLoader or
 * GlycinNgImage across threads without external synchronization.
 *
 * Lifetimes:
 *   - GlycinNgLoader and GlycinNgImage are heap-allocated. Free them
 *     with glycin_ng_loader_free or glycin_ng_image_free.
 *   - Pointers returned by glycin_ng_image_texture,
 *     glycin_ng_image_format_name, and glycin_ng_texture_data
 *     remain valid until the owning GlycinNgImage is freed.
 *   - glycin_ng_last_error returns a pointer valid until the next
 *     call on the same thread that produces or clears an error.
 *
 * Error reporting:
 *   - Constructors and decode functions return NULL on failure.
 *   - Setters return 0 on success and a negative value on failure.
 *   - On any failure, glycin_ng_last_error() returns a UTF-8
 *     NUL-terminated message describing what went wrong.
 */
#ifndef GLYCIN_NG_H
#define GLYCIN_NG_H

#include <stddef.h>
#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

typedef struct GlycinNgLoader GlycinNgLoader;
typedef struct GlycinNgImage GlycinNgImage;
typedef struct GlycinNgTexture GlycinNgTexture;

/* Texture pixel format constants. Match the Rust MemoryFormat. */
#define GLYCIN_NG_FORMAT_UNKNOWN 0u
#define GLYCIN_NG_FORMAT_G8 1u
#define GLYCIN_NG_FORMAT_G8A8 2u
#define GLYCIN_NG_FORMAT_G8A8_PRE 3u
#define GLYCIN_NG_FORMAT_G16 4u
#define GLYCIN_NG_FORMAT_G16A16 5u
#define GLYCIN_NG_FORMAT_G16A16_PRE 6u
#define GLYCIN_NG_FORMAT_R8G8B8 10u
#define GLYCIN_NG_FORMAT_R8G8B8A8 11u
#define GLYCIN_NG_FORMAT_R8G8B8A8_PRE 12u
#define GLYCIN_NG_FORMAT_B8G8R8 13u
#define GLYCIN_NG_FORMAT_B8G8R8A8 14u
#define GLYCIN_NG_FORMAT_B8G8R8A8_PRE 15u
#define GLYCIN_NG_FORMAT_A8R8G8B8 16u
#define GLYCIN_NG_FORMAT_A8R8G8B8_PRE 17u
#define GLYCIN_NG_FORMAT_A8B8G8R8 18u
#define GLYCIN_NG_FORMAT_R16G16B16 20u
#define GLYCIN_NG_FORMAT_R16G16B16A16 21u
#define GLYCIN_NG_FORMAT_R16G16B16A16_PRE 22u
#define GLYCIN_NG_FORMAT_R16G16B16_F 23u
#define GLYCIN_NG_FORMAT_R16G16B16A16_F 24u
#define GLYCIN_NG_FORMAT_R32G32B32_F 25u
#define GLYCIN_NG_FORMAT_R32G32B32A32_F 26u
#define GLYCIN_NG_FORMAT_R32G32B32A32_F_PRE 27u

/* Known-format constants for glycin_ng_loader_format_hint. */
#define GLYCIN_NG_KFMT_PNG 1u
#define GLYCIN_NG_KFMT_JPEG 2u
#define GLYCIN_NG_KFMT_GIF 3u
#define GLYCIN_NG_KFMT_WEBP 4u
#define GLYCIN_NG_KFMT_TIFF 5u
#define GLYCIN_NG_KFMT_BMP 6u
#define GLYCIN_NG_KFMT_ICO 7u
#define GLYCIN_NG_KFMT_TGA 8u
#define GLYCIN_NG_KFMT_QOI 9u
#define GLYCIN_NG_KFMT_EXR 10u
#define GLYCIN_NG_KFMT_PNM 11u
#define GLYCIN_NG_KFMT_DDS 12u
#define GLYCIN_NG_KFMT_JXL 13u

/* Error helpers. */
const char* glycin_ng_last_error(void);
void glycin_ng_clear_last_error(void);

/* Loader lifecycle. */
GlycinNgLoader* glycin_ng_loader_new_path(const char* path);
GlycinNgLoader* glycin_ng_loader_new_bytes(const uint8_t* data, size_t len);
void glycin_ng_loader_free(GlycinNgLoader* loader);

/* Loader configuration. */
int glycin_ng_loader_sandbox(GlycinNgLoader* loader,
                             int landlock, int seccomp,
                             int rlimit, int strict);
int glycin_ng_loader_format_hint(GlycinNgLoader* loader, unsigned int format);

/* Decode. Consumes the loader regardless of success. */
GlycinNgImage* glycin_ng_loader_load(GlycinNgLoader* loader);

/* Image accessors. */
void glycin_ng_image_free(GlycinNgImage* image);
uint32_t glycin_ng_image_width(const GlycinNgImage* image);
uint32_t glycin_ng_image_height(const GlycinNgImage* image);
size_t glycin_ng_image_frame_count(const GlycinNgImage* image);
const char* glycin_ng_image_format_name(const GlycinNgImage* image);
const GlycinNgTexture* glycin_ng_image_texture(const GlycinNgImage* image,
                                               size_t index);
uint64_t glycin_ng_image_frame_delay_ms(const GlycinNgImage* image,
                                        size_t index);

/* Texture accessors. */
uint32_t glycin_ng_texture_width(const GlycinNgTexture* texture);
uint32_t glycin_ng_texture_height(const GlycinNgTexture* texture);
uint32_t glycin_ng_texture_stride(const GlycinNgTexture* texture);
unsigned int glycin_ng_texture_format(const GlycinNgTexture* texture);
const uint8_t* glycin_ng_texture_data(const GlycinNgTexture* texture);
size_t glycin_ng_texture_data_len(const GlycinNgTexture* texture);

#ifdef __cplusplus
}
#endif
#endif /* GLYCIN_NG_H */
