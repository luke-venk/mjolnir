#include "libintl.h"

/* --- Standard Gettext Stubs --- */
char *gettext(const char *msgid) { return (char *)msgid; }
char *dgettext(const char *domain, const char *msgid) { return (char *)msgid; }
char *dcgettext(const char *domain, const char *msgid, int category) { return (char *)msgid; }
char *ngettext(const char *m1, const char *m2, unsigned long n) { return (n == 1) ? (char *)m1 : (char *)m2; }
char *dngettext(const char *domain, const char *m1, const char *m2, unsigned long n) { return (n == 1) ? (char *)m1 : (char *)m2; }
char *dcngettext(const char *domain, const char *m1, const char *m2, unsigned long n, int category) { return (n == 1) ? (char *)m1 : (char *)m2; }
char *bindtextdomain(const char *domain, const char *dir) { return (char *)domain; }
char *bind_textdomain_codeset(const char *domain, const char *codeset) { return (char *)codeset; }
char *textdomain(const char *domain) { return (char *)domain; }

/* --- GLib Specific Gettext Stubs --- */
const gchar *g_dgettext(const gchar *domain, const gchar *msgid) { return msgid; }
const gchar *g_dngettext(const gchar *domain, const gchar *m1, const gchar *m2, gulong n)
{
  return (n == 1) ? m1 : m2;
}
const gchar *g_dcgettext(const gchar *domain, const gchar *msgid, int category) { return msgid; }

/* --- macOS Platform Stubs --- */
#ifdef __APPLE__
void load_user_special_dirs_macos(gchar **special_dirs) { /* No-op */ }
const gchar *g_content_type_get_icon_impl(const gchar *type) { return NULL; }
const gchar *g_content_type_get_symbolic_icon_impl(const gchar *type) { return NULL; }
const gchar *g_content_type_from_mime_type_impl(const gchar *mime) { return NULL; }
const gchar *g_content_type_guess_impl(const gchar *filename, const guchar *data, size_t data_size, int *result_uncertain) { return NULL; }
#endif

/* --- GIO Internal Stubs --- */
int glib_get_sandbox_type(void) { return 0; }
gsize g_threaded_resolver_get_type(void) { return 0; }
gsize _g_local_vfs_get_type(void) { return 0; }
gpointer _g_local_vfs_new(void) { return NULL; }

/* Additional GIO Module Stubs */
gsize g_memory_monitor_portal_get_type(void) { return 0; }
gsize g_memory_monitor_dbus_get_type(void) { return 0; }
gsize g_memory_monitor_get_type(void) { return 0; }
gsize g_fdo_notification_backend_get_type(void) { return 0; }
gsize g_osx_app_info_get_type(void) { return 0; }
gsize g_nextstep_settings_backend_get_type(void) { return 0; }
gsize g_cocoa_notification_backend_get_type(void) { return 0; }
gsize g_kqueue_file_monitor_get_type(void) { return 0; }
gsize g_keyfile_settings_backend_get_type(void) { return 0; }
