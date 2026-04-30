#ifndef LIBINTL_H
#define LIBINTL_H

#include <stddef.h>
#include "glib/glib/gtypes.h"

/* Standard Gettext */
char *gettext(const char *msgid);
char *dgettext(const char *domain, const char *msgid);
char *dcgettext(const char *domain, const char *msgid, int category);
char *ngettext(const char *msg1, const char *msg2, unsigned long n);
char *dngettext(const char *domain, const char *msg1, const char *msg2, unsigned long n);
char *dcngettext(const char *domain, const char *msg1, const char *msg2, unsigned long n, int category);
char *bindtextdomain(const char *domain, const char *dir);
char *bind_textdomain_codeset(const char *domain, const char *codeset);
char *textdomain(const char *domain);

/* GLib Specific Gettext - declarations must exist for the C files to compile */
const gchar *g_dgettext(const gchar *domain, const gchar *msgid);
const gchar *g_dcgettext(const gchar *domain, const gchar *msgid, int category);
const gchar *g_dngettext(const gchar *domain,
                         const gchar *msgid1,
                         const gchar *msgid2,
                         gulong n);

/* Platform Impl Stubs */
#ifdef __APPLE__
void load_user_special_dirs_macos(gchar **special_dirs);
const gchar *g_content_type_get_icon_impl(const gchar *type);
const gchar *g_content_type_get_symbolic_icon_impl(const gchar *type);
const gchar *g_content_type_from_mime_type_impl(const gchar *mime);
const gchar *g_content_type_guess_impl(const gchar *filename, const guchar *data, size_t data_size, int *result_uncertain);
#endif

/* GIO/Sandbox Stubs */
int glib_get_sandbox_type(void);
gsize g_threaded_resolver_get_type(void);
gsize _g_local_vfs_get_type(void);
gpointer _g_local_vfs_new(void);

/* Additional GIO Module Stubs */
gsize g_memory_monitor_portal_get_type(void);
gsize g_memory_monitor_dbus_get_type(void);
gsize g_memory_monitor_get_type(void);
gsize g_fdo_notification_backend_get_type(void);
gsize g_osx_app_info_get_type(void);
gsize g_nextstep_settings_backend_get_type(void);
gsize g_cocoa_notification_backend_get_type(void);
gsize g_kqueue_file_monitor_get_type(void);
gsize g_keyfile_settings_backend_get_type(void);

#endif
