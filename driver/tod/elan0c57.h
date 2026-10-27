/* SPDX-License-Identifier: LGPL-2.1-or-later */
/* C interface of the Rust library (src/ffi.rs). */
#pragma once

#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>

#define ELAN0C57_OK         0
#define ELAN0C57_CANCELLED  1
#define ELAN0C57_PARTIAL    2
#define ELAN0C57_RETRY      3
#define ELAN0C57_ERROR     -1

typedef struct Elan Elan;
typedef bool (*Elan0c57CancelFn) (void *user_data);

Elan  *elan0c57_open (char **error);
void   elan0c57_close (Elan *dev);
int    elan0c57_capture (Elan *dev, Elan0c57CancelFn cancel, void *user_data,
                         uint8_t **out, size_t *out_len, char **error);
void   elan0c57_wait_finger_off (Elan *dev);
int    elan0c57_merge (const uint8_t * const *touches, const size_t *lens, size_t n,
                       uint8_t **out, size_t *out_len);
double elan0c57_score (Elan *dev, const uint8_t *enrolled, size_t enrolled_len,
                       const uint8_t *probe, size_t probe_len);
double elan0c57_threshold (void);
void   elan0c57_free (uint8_t *data, size_t len);
void   elan0c57_free_string (char *text);
