// SPDX-License-Identifier: LGPL-2.1-or-later
/* libcrypto.so.1.1 shim: exposes the few OpenSSL 1.1 EVP symbols used by the
 * ELAN TOD driver (with the OPENSSL_1_1_0 version) forwarding to OpenSSL 3. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdlib.h>
#include <stdio.h>

static void *real;
static void *sym (const char *name)
{
  if (!real && !(real = dlopen ("libcrypto.so.3", RTLD_NOW | RTLD_LOCAL)))
    { fprintf (stderr, "crypto-compat: %s\n", dlerror ()); abort (); }
  void *s = dlsym (real, name);
  if (!s) { fprintf (stderr, "crypto-compat: missing %s\n", name); abort (); }
  return s;
}
#define FWD(ret, name, proto, args) \
  ret name proto { static ret (*f) proto; if (!f) f = sym (#name); return f args; }

FWD (const void *, EVP_aes_256_cbc, (void), ())
FWD (const void *, EVP_sha256, (void), ())
FWD (int, EVP_BytesToKey, (const void *t, const void *md, const unsigned char *salt, const unsigned char *data, int datal, int count, unsigned char *key, unsigned char *iv), (t, md, salt, data, datal, count, key, iv))
FWD (void *, EVP_CIPHER_CTX_new, (void), ())
FWD (int, EVP_CipherInit_ex, (void *ctx, const void *t, void *impl, const unsigned char *key, const unsigned char *iv, int enc), (ctx, t, impl, key, iv, enc))
FWD (int, EVP_CipherUpdate, (void *ctx, unsigned char *out, int *outl, const unsigned char *in, int inl), (ctx, out, outl, in, inl))
FWD (int, EVP_CipherFinal_ex, (void *ctx, unsigned char *out, int *outl), (ctx, out, outl))

void EVP_CIPHER_CTX_free (void *ctx) { static void (*f) (void *); if (!f) f = sym ("EVP_CIPHER_CTX_free"); f (ctx); }
/* became EVP_CIPHER_get_block_size in OpenSSL 3 */
int EVP_CIPHER_block_size (const void *c) { static int (*f) (const void *); if (!f) f = sym ("EVP_CIPHER_get_block_size"); return f (c); }
