#ifndef ARV_VERSION_H
#define ARV_VERSION_H
#if !defined(ARV_H_INSIDE) && !defined(ARAVIS_COMPILATION)
#error "Only <arv.h> can be included directly."
#endif
#include <arvtypes.h>
G_BEGIN_DECLS
#define ARAVIS_VERSION "0.9.2"
#define ARAVIS_API_VERSION "0.10"
#define ARAVIS_MAJOR_VERSION 0
#define ARAVIS_MINOR_VERSION 9
#define ARAVIS_MICRO_VERSION 2
#define ARAVIS_CHECK_VERSION(major, minor, micro)                         \
  (ARAVIS_MAJOR_VERSION > (major) ||                                      \
   (ARAVIS_MAJOR_VERSION == (major) && ARAVIS_MINOR_VERSION > (minor)) || \
   (ARAVIS_MAJOR_VERSION == (major) && ARAVIS_MINOR_VERSION == (minor) && \
    ARAVIS_MICRO_VERSION >= (micro)))
G_END_DECLS
#endif
