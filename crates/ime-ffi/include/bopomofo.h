#ifndef BOPOMOFO_H
#define BOPOMOFO_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#define BPM_ABI_VERSION 2
#ifdef __cplusplus
extern "C" {
#endif
typedef struct BpmEngine BpmEngine;
typedef struct BpmBuffer { uint8_t *data; size_t len; } BpmBuffer;
typedef struct BpmAsset {
    const uint8_t *name;
    size_t name_len;
    const uint8_t *data;
    size_t len;
} BpmAsset;
enum { BPM_OK=0, BPM_INVALID=1, BPM_DATA=2, BPM_PANIC=3 };
uint32_t bpm_abi_version(void);
/* v2 requires exactly one asset named dictionary.dat, <=32 MiB, and an
   assets.json manifest <=16 KiB. Inputs are borrowed for this call only.
   The manifest fixes the engine/builder/data versions, size, and SHA-256.
   Native binaries must check BPM_ABI_VERSION before calling any other API.
   A v1 constructor symbol is intentionally not provided. */
int32_t bpm_assets_verify(const uint8_t *manifest, size_t len, const BpmAsset *assets, size_t count);
/* out must point to an initialized NULL handle. Initialization copies all
   required bytes. Inputs and outputs must not overlap. Calls on a handle
   must be serialized. Non-NULL pointers must address valid regions of the
   specified lengths; parameter checks cannot establish pointer validity. */
int32_t bpm_engine_new_v2(const uint8_t *manifest, size_t len, const BpmAsset *assets, size_t count, BpmEngine **out);
/* UTF-8 event/key JSON <=4096 bytes. Initialize output buffers to {NULL,0}.
   Each returned allocation is freed exactly once using its matching API.
   NULL free is allowed. A panic poisons the context: discard and recreate it.
   Never retry an event or native effect that may already have committed. */
int32_t bpm_engine_handle(BpmEngine *engine, const uint8_t *event_json, size_t len, BpmBuffer *out);
int32_t bpm_engine_snapshot(const BpmEngine *engine, BpmBuffer *out);
int32_t bpm_engine_probe(const BpmEngine *engine, const uint8_t *key_json, size_t len, bool *out);
void bpm_engine_free(BpmEngine *engine);
void bpm_buffer_free(BpmBuffer buffer);
#ifdef __cplusplus
}
#endif
#endif
