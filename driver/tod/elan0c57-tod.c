/* SPDX-License-Identifier: LGPL-2.1-or-later */
/*
 * libfprint-TOD driver for the ELAN 04f3:0c57 press fingerprint reader.
 *
 * A thin adapter: capture, image processing and matching are done by the
 * Rust library (elan0c57.h, src/ffi.rs). Its calls block, so they run in a
 * GTask worker thread; the results are reported to libfprint from the main
 * context.
 *
 * This library is free software; you can redistribute it and/or modify it
 * under the terms of the GNU Lesser General Public License as published by
 * the Free Software Foundation; either version 2.1 of the License, or (at
 * your option) any later version.
 */

#define FP_COMPONENT "elan0c57rs"

#include <gio/gio.h>
#include <gmodule.h>

#include "fpi-device.h"
#include "fpi-log.h"
#include "fpi-print.h"

#include "elan0c57.h"

#define ENROLL_STAGES 12

struct _FpiDeviceElan0c57
{
  FpDevice   parent;
  Elan      *elan;
  GPtrArray *touches;   /* GBytes of the touches captured while enrolling */
};

G_DECLARE_FINAL_TYPE (FpiDeviceElan0c57, fpi_device_elan0c57, FPI, DEVICE_ELAN0C57, FpDevice)
G_DEFINE_TYPE (FpiDeviceElan0c57, fpi_device_elan0c57, FP_TYPE_DEVICE)

static const FpIdEntry id_table[] = {
  { .vid = 0x04f3, .pid = 0x0c57 },
  { .vid = 0, .pid = 0 },
};

/* -- worker thread ------------------------------------------------------- */

typedef struct
{
  gboolean   wait_finger_off;  /* first wait for the previous finger to leave */
  GPtrArray *gallery;          /* GBytes of enrolled prints to score, or NULL */
} Job;

typedef struct
{
  int     status;              /* ELAN0C57_* */
  GBytes *touch;
  char   *error;
  int     best;                /* index in the gallery, -1 if none */
  double  score;
} Result;

static void
job_free (Job *job)
{
  g_clear_pointer (&job->gallery, g_ptr_array_unref);
  g_free (job);
}

static void
result_free (Result *r)
{
  g_clear_pointer (&r->touch, g_bytes_unref);
  g_free (r->error);
  g_free (r);
}

static bool
is_cancelled (void *cancellable)
{
  return g_cancellable_is_cancelled (cancellable);
}

static void
worker (GTask *task, gpointer source, gpointer task_data, GCancellable *cancellable)
{
  FpiDeviceElan0c57 *self = source;
  Job *job = task_data;
  Result *r = g_new0 (Result, 1);
  uint8_t *buf = NULL;
  size_t len = 0;
  char *error = NULL;

  r->best = -1;
  r->score = -1.0;

  if (job->wait_finger_off)
    elan0c57_wait_finger_off (self->elan);

  r->status = elan0c57_capture (self->elan, is_cancelled, cancellable, &buf, &len, &error);
  if (error)
    {
      r->error = g_strdup (error);
      elan0c57_free_string (error);
    }
  if (r->status == ELAN0C57_OK)
    {
      r->touch = g_bytes_new (buf, len);
      elan0c57_free (buf, len);
    }

  if (r->status == ELAN0C57_OK && job->gallery)
    for (guint i = 0; i < job->gallery->len; i++)
      {
        gsize enrolled_len;
        const uint8_t *enrolled = g_bytes_get_data (g_ptr_array_index (job->gallery, i), &enrolled_len);
        gsize probe_len;
        const uint8_t *probe = g_bytes_get_data (r->touch, &probe_len);
        double score = elan0c57_score (self->elan, enrolled, enrolled_len, probe, probe_len);

        fp_dbg ("print %u: score %.3f", i, score);
        if (score > r->score)
          {
            r->score = score;
            r->best = i;
          }
      }

  g_task_return_pointer (task, r, (GDestroyNotify) result_free);
}

static void
start (FpiDeviceElan0c57 *self, gboolean wait_finger_off, GPtrArray *gallery, GAsyncReadyCallback callback)
{
  g_autoptr(GTask) task = g_task_new (self, fpi_device_get_cancellable (FP_DEVICE (self)), callback, NULL);
  Job *job = g_new0 (Job, 1);

  job->wait_finger_off = wait_finger_off;
  job->gallery = gallery;
  g_task_set_task_data (task, job, (GDestroyNotify) job_free);
  g_task_run_in_thread (task, worker);
}

/* The data of an enrolled print, as stored by fprintd. */
static GBytes *
print_data (FpPrint *print)
{
  g_autoptr(GVariant) data = NULL;
  const guint8 *bytes;
  gsize len = 0;

  g_object_get (print, "fpi-data", &data, NULL);
  if (!data)
    return g_bytes_new (NULL, 0);
  bytes = g_variant_get_fixed_array (data, &len, 1);
  return g_bytes_new (bytes, len);
}

static GError *
result_error (Result *r)
{
  switch (r->status)
    {
    case ELAN0C57_PARTIAL:
      return fpi_device_retry_new (FP_DEVICE_RETRY_CENTER_FINGER);

    case ELAN0C57_RETRY:
      return fpi_device_retry_new (FP_DEVICE_RETRY_REMOVE_FINGER);

    case ELAN0C57_CANCELLED:
      return g_error_new_literal (G_IO_ERROR, G_IO_ERROR_CANCELLED, "Operation was cancelled");

    default:
      return fpi_device_error_new_msg (FP_DEVICE_ERROR_PROTO, "%s", r->error ? r->error : "unknown error");
    }
}

static gboolean
is_retry (Result *r)
{
  return r->status == ELAN0C57_PARTIAL || r->status == ELAN0C57_RETRY;
}

/* -- enroll -------------------------------------------------------------- */

static void
enroll_done (FpiDeviceElan0c57 *self)
{
  FpDevice *dev = FP_DEVICE (self);
  g_autofree const uint8_t **touches = g_new (const uint8_t *, self->touches->len);
  g_autofree size_t *lens = g_new (size_t, self->touches->len);
  FpPrint *print = NULL;
  uint8_t *buf = NULL;
  size_t len = 0;
  int status;

  for (guint i = 0; i < self->touches->len; i++)
    {
      gsize l;
      touches[i] = g_bytes_get_data (g_ptr_array_index (self->touches, i), &l);
      lens[i] = l;
    }
  status = elan0c57_merge (touches, lens, self->touches->len, &buf, &len);
  g_clear_pointer (&self->touches, g_ptr_array_unref);

  if (status != ELAN0C57_OK)
    {
      fpi_device_enroll_complete (dev, NULL,
                                  fpi_device_error_new_msg (FP_DEVICE_ERROR_GENERAL, "could not build the print"));
      return;
    }

  fpi_device_get_enroll_data (dev, &print);
  fpi_print_set_type (print, FPI_PRINT_RAW);
  g_object_set (print, "fpi-data", g_variant_new_fixed_array (G_VARIANT_TYPE_BYTE, buf, len, 1), NULL);
  elan0c57_free (buf, len);
  fpi_device_enroll_complete (dev, g_object_ref (print), NULL);
}

static void
enroll_cb (GObject *source, GAsyncResult *res, gpointer user_data)
{
  FpiDeviceElan0c57 *self = FPI_DEVICE_ELAN0C57 (source);
  FpDevice *dev = FP_DEVICE (self);
  g_autoptr(GError) error = NULL;
  Result *r = g_task_propagate_pointer (G_TASK (res), &error);

  if (!r)
    {
      g_clear_pointer (&self->touches, g_ptr_array_unref);
      fpi_device_enroll_complete (dev, NULL, g_steal_pointer (&error));
      return;
    }

  if (r->status == ELAN0C57_OK)
    {
      g_ptr_array_add (self->touches, g_bytes_ref (r->touch));
      fpi_device_enroll_progress (dev, self->touches->len, NULL, NULL);
    }
  else if (is_retry (r))
    {
      fpi_device_enroll_progress (dev, self->touches->len, NULL, result_error (r));
    }
  else
    {
      g_clear_pointer (&self->touches, g_ptr_array_unref);
      fpi_device_enroll_complete (dev, NULL, result_error (r));
      result_free (r);
      return;
    }
  result_free (r);

  if (self->touches->len < ENROLL_STAGES)
    start (self, TRUE, NULL, enroll_cb);
  else
    enroll_done (self);
}

static void
enroll (FpDevice *dev)
{
  FpiDeviceElan0c57 *self = FPI_DEVICE_ELAN0C57 (dev);

  g_clear_pointer (&self->touches, g_ptr_array_unref);
  self->touches = g_ptr_array_new_with_free_func ((GDestroyNotify) g_bytes_unref);
  start (self, FALSE, NULL, enroll_cb);
}

/* -- verify and identify ------------------------------------------------- */

static void
verify_cb (GObject *source, GAsyncResult *res, gpointer user_data)
{
  FpDevice *dev = FP_DEVICE (source);
  g_autoptr(GError) error = NULL;
  Result *r = g_task_propagate_pointer (G_TASK (res), &error);

  if (!r)
    {
      fpi_device_verify_complete (dev, g_steal_pointer (&error));
      return;
    }
  if (r->status == ELAN0C57_OK)
    {
      fp_dbg ("verify score %.3f, threshold %.3f", r->score, elan0c57_threshold ());
      fpi_device_verify_report (dev, r->score >= elan0c57_threshold () ? FPI_MATCH_SUCCESS : FPI_MATCH_FAIL,
                                NULL, NULL);
      fpi_device_verify_complete (dev, NULL);
    }
  else if (is_retry (r))
    {
      fpi_device_verify_report (dev, FPI_MATCH_ERROR, NULL, result_error (r));
      fpi_device_verify_complete (dev, NULL);
    }
  else
    {
      fpi_device_verify_complete (dev, result_error (r));
    }
  result_free (r);
}

static void
verify (FpDevice *dev)
{
  FpPrint *print = NULL;
  GPtrArray *gallery = g_ptr_array_new_with_free_func ((GDestroyNotify) g_bytes_unref);

  fpi_device_get_verify_data (dev, &print);
  g_ptr_array_add (gallery, print_data (print));
  start (FPI_DEVICE_ELAN0C57 (dev), FALSE, gallery, verify_cb);
}

static void
identify_cb (GObject *source, GAsyncResult *res, gpointer user_data)
{
  FpDevice *dev = FP_DEVICE (source);
  g_autoptr(GError) error = NULL;
  Result *r = g_task_propagate_pointer (G_TASK (res), &error);
  GPtrArray *prints = NULL;

  if (!r)
    {
      fpi_device_identify_complete (dev, g_steal_pointer (&error));
      return;
    }
  if (r->status == ELAN0C57_OK)
    {
      FpPrint *match = NULL;

      fpi_device_get_identify_data (dev, &prints);
      fp_dbg ("identify best score %.3f (print %d), threshold %.3f", r->score, r->best, elan0c57_threshold ());
      if (r->best >= 0 && r->score >= elan0c57_threshold ())
        match = g_ptr_array_index (prints, r->best);
      fpi_device_identify_report (dev, match, NULL, NULL);
      fpi_device_identify_complete (dev, NULL);
    }
  else if (is_retry (r))
    {
      fpi_device_identify_report (dev, NULL, NULL, result_error (r));
      fpi_device_identify_complete (dev, NULL);
    }
  else
    {
      fpi_device_identify_complete (dev, result_error (r));
    }
  result_free (r);
}

static void
identify (FpDevice *dev)
{
  GPtrArray *prints = NULL;
  GPtrArray *gallery = g_ptr_array_new_with_free_func ((GDestroyNotify) g_bytes_unref);

  fpi_device_get_identify_data (dev, &prints);
  for (guint i = 0; i < prints->len; i++)
    g_ptr_array_add (gallery, print_data (g_ptr_array_index (prints, i)));
  start (FPI_DEVICE_ELAN0C57 (dev), FALSE, gallery, identify_cb);
}

/* -- open and close ------------------------------------------------------ */

static void
dev_open (FpDevice *dev)
{
  FpiDeviceElan0c57 *self = FPI_DEVICE_ELAN0C57 (dev);
  char *error = NULL;

  self->elan = elan0c57_open (&error);
  if (!self->elan)
    {
      fpi_device_open_complete (dev, fpi_device_error_new_msg (FP_DEVICE_ERROR_GENERAL, "%s", error));
      elan0c57_free_string (error);
      return;
    }
  fpi_device_open_complete (dev, NULL);
}

static void
dev_close (FpDevice *dev)
{
  FpiDeviceElan0c57 *self = FPI_DEVICE_ELAN0C57 (dev);

  g_clear_pointer (&self->elan, elan0c57_close);
  fpi_device_close_complete (dev, NULL);
}

static void
fpi_device_elan0c57_init (FpiDeviceElan0c57 *self)
{
}

static void
fpi_device_elan0c57_class_init (FpiDeviceElan0c57Class *klass)
{
  FpDeviceClass *dev_class = FP_DEVICE_CLASS (klass);

  dev_class->id = "elan0c57rs";
  dev_class->full_name = "ELAN 04f3:0c57 (open source)";
  dev_class->type = FP_DEVICE_TYPE_USB;
  dev_class->id_table = id_table;
  dev_class->nr_enroll_stages = ENROLL_STAGES;
  dev_class->scan_type = FP_SCAN_TYPE_PRESS;

  dev_class->open = dev_open;
  dev_class->close = dev_close;
  dev_class->enroll = enroll;
  dev_class->verify = verify;
  dev_class->identify = identify;
}

G_MODULE_EXPORT GType
fpi_tod_shared_driver_get_type (void)
{
  return fpi_device_elan0c57_get_type ();
}
