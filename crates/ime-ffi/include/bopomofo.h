#ifndef BOPOMOFO_H
#define BOPOMOFO_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
#define BPM_ABI_VERSION 1
#ifdef __cplusplus
extern "C" {
#endif
typedef struct BpmEngine BpmEngine;
typedef struct BpmBuffer { uint8_t *data; size_t len; } BpmBuffer;
enum { BPM_OK=0, BPM_INVALID=1, BPM_DATA=2, BPM_PANIC=3 };
uint32_t bpm_abi_version(void);
/* Verify bytes against a 64-digit ASCII SHA-256 manifest before context creation. */
int32_t bpm_lexicon_verify(const uint8_t *lexicon, size_t len, const uint8_t *sha256, size_t sha256_len);
/* Input buffers are borrowed for this call only; UTF-8, lexicon <=32 MiB,
   event/key JSON <=4096 bytes. Outputs must be initialized to NULL / {NULL,0}.
   Calls on a handle must be serialized. Each allocation is freed exactly once.
   NULL free is allowed. A panic poisons the context: discard and recreate it.
   Non-NULL pointers must address valid memory of the specified length. */
int32_t bpm_engine_new(const uint8_t *lexicon, size_t len, BpmEngine **out);
int32_t bpm_engine_handle(BpmEngine *engine, const uint8_t *event_json, size_t len, BpmBuffer *out);
int32_t bpm_engine_snapshot(const BpmEngine *engine, BpmBuffer *out);
int32_t bpm_engine_probe(const BpmEngine *engine, const uint8_t *key_json, size_t len, bool *out);
void bpm_engine_free(BpmEngine *engine);
void bpm_buffer_free(BpmBuffer buffer);
#ifdef __cplusplus
}
#endif
#endif
