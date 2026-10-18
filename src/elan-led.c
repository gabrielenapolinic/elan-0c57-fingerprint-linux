// SPDX-License-Identifier: LGPL-2.1-or-later
/* LED support for the ELAN TOD driver on the 0c57: the driver never sends the
 * "led on" command (0x40 0x31) used by the open elan driver, so its bulk
 * transfers are routed here (import renamed to e_usb_device_bulk_transfer_async)
 * and the LED is switched on right before the "wait for finger" command
 * (0x40 0x3f). Like the open driver does after each capture, the LED is
 * switched off (0x00 0x0b) once the finger image has been read, right before
 * the driver waits for the finger to be lifted (0x40 0x3e), so it blinks once
 * per touch. */
#include <gusb.h>

#define ELAN_EP_CMD_OUT 0x01

void
e_usb_device_bulk_transfer_async (GUsbDevice *device, guint8 endpoint,
                                  guint8 *data, gsize length, guint timeout,
                                  GCancellable *cancellable,
                                  GAsyncReadyCallback callback, gpointer user_data)
{
  if (g_getenv ("ELAN_LED_TRACE"))
    g_printerr ("elan-trace %" G_GINT64_FORMAT " ep=0x%02x len=%zu %s%02x %02x\n",
                g_get_monotonic_time () / 1000, endpoint, length,
                (endpoint & 0x80) ? "IN " : "OUT ",
                (endpoint & 0x80) ? 0 : data[0], (endpoint & 0x80 || length < 2) ? 0 : data[1]);

  if (endpoint == ELAN_EP_CMD_OUT && length == 2 && data[0] == 0x40 && data[1] == 0x3f)
    {
      guint8 led_on[] = { 0x40, 0x31 };
      g_autoptr(GError) error = NULL;

      if (!g_usb_device_bulk_transfer (device, ELAN_EP_CMD_OUT, led_on, sizeof (led_on),
                                       NULL, 1000, NULL, &error))
        g_debug ("elan-led: led on failed: %s", error->message);
    }

  else if (endpoint == ELAN_EP_CMD_OUT && length == 2 && data[0] == 0x40 && data[1] == 0x3e)
    {
      guint8 led_off[] = { 0x00, 0x0b };
      g_autoptr(GError) error = NULL;

      if (!g_usb_device_bulk_transfer (device, ELAN_EP_CMD_OUT, led_off, sizeof (led_off),
                                       NULL, 1000, NULL, &error))
        g_debug ("elan-led: led off failed: %s", error->message);
    }

  g_usb_device_bulk_transfer_async (device, endpoint, data, length, timeout,
                                    cancellable, callback, user_data);
}
