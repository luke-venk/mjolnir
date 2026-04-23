#ifndef LIBINTL_H
#define LIBINTL_H

#include <stddef.h>
#include "glib/glib/gtypes.h"

char *gettext(const char *msgid);
char *dgettext(const char *domain, const char *msgid);
char *dcgettext(const char *domain, const char *msgid, int category);
char *ngettext(const char *msg1, const char *msg2, unsigned long n);
char *dngettext(const char *domain, const char *msg1, const char *msg2, unsigned long n);
char *dcngettext(const char *domain, const char *msg1, const char *msg2, unsigned long n, int category);
char *bindtextdomain(const char *domain, const char *dir);
char *bind_textdomain_codeset(const char *domain, const char *codeset);
char *textdomain(const char *domain);

const gchar *g_dngettext(const gchar *domain,
                         const gchar *msgid1,
                         const gchar *msgid2,
                         gulong n);

#endif