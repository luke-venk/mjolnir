#include "libintl.h"

char *gettext(const char *msgid) { return (char *)msgid; }
char *dgettext(const char *domain, const char *msgid) { return (char *)msgid; }
char *dcgettext(const char *domain, const char *msgid, int category) { return (char *)msgid; }
char *ngettext(const char *m1, const char *m2, unsigned long n) { return (n == 1) ? (char *)m1 : (char *)m2; }
char *dngettext(const char *domain, const char *m1, const char *m2, unsigned long n) { return (n == 1) ? (char *)m1 : (char *)m2; }
char *dcngettext(const char *domain, const char *m1, const char *m2, unsigned long n, int category) { return (n == 1) ? (char *)m1 : (char *)m2; }
char *bindtextdomain(const char *domain, const char *dir) { return (char *)domain; }
char *bind_textdomain_codeset(const char *domain, const char *codeset) { return (char *)codeset; }
char *textdomain(const char *domain) { return (char *)domain; }

const gchar *g_dngettext(const gchar *domain,
                         const gchar *msgid1,
                         const gchar *msgid2,
                         gulong n)
{
  return (n == 1) ? msgid1 : msgid2;
}