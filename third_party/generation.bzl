"""Helpers for generating glib headers"""

def generate_visibility_header(name, namespace, version, out):
    """
    Generates the visibility headers for glib/ libraries

    Args:
        name: fuck
        namespace: fuck
        version: fuck
        out: fuck
    """
    minor_version = int(version.split(".")[1])

    content = ["""#pragma once
#if (defined(_WIN32) || defined(__CYGWIN__)) && !defined({ns}_STATIC_COMPILATION)
#  define _{ns}_EXPORT __declspec(dllexport)
#  define _{ns}_IMPORT __declspec(dllimport)
#elif __GNUC__ >= 4
#  define _{ns}_EXPORT __attribute__((visibility("default")))
#  define _{ns}_IMPORT
#else
#  define _{ns}_EXPORT
#  define _{ns}_IMPORT
#endif
#ifdef {ns}_COMPILATION
#  define _{ns}_API _{ns}_EXPORT
#else
#  define _{ns}_API _{ns}_IMPORT
#endif
#define _{ns}_EXTERN _{ns}_API extern
#define {ns}_VAR _{ns}_EXTERN
#define {ns}_AVAILABLE_IN_ALL _{ns}_EXTERN
#ifdef GLIB_DISABLE_DEPRECATION_WARNINGS
#define {ns}_DEPRECATED _{ns}_EXTERN
#define {ns}_DEPRECATED_FOR(f) _{ns}_EXTERN
#define {ns}_UNAVAILABLE(maj,min) _{ns}_EXTERN
#define {ns}_UNAVAILABLE_STATIC_INLINE(maj,min)
#else
#define {ns}_DEPRECATED G_DEPRECATED _{ns}_EXTERN
#define {ns}_DEPRECATED_FOR(f) G_DEPRECATED_FOR(f) _{ns}_EXTERN
#define {ns}_UNAVAILABLE(maj,min) G_UNAVAILABLE(maj,min) _{ns}_EXTERN
#define {ns}_UNAVAILABLE_STATIC_INLINE(maj,min) G_UNAVAILABLE(maj,min)
#endif
""".format(ns = namespace)]

    for m in range(26, minor_version + 4, 2):
        content.append("""
#if GLIB_VERSION_MIN_REQUIRED >= GLIB_VERSION_2_{minor}
#define {ns}_DEPRECATED_IN_2_{minor} {ns}_DEPRECATED
#define {ns}_DEPRECATED_IN_2_{minor}_FOR(f) {ns}_DEPRECATED_FOR (f)
#define {ns}_DEPRECATED_MACRO_IN_2_{minor} GLIB_DEPRECATED_MACRO
#define {ns}_DEPRECATED_MACRO_IN_2_{minor}_FOR(f) GLIB_DEPRECATED_MACRO_FOR (f)
#define {ns}_DEPRECATED_ENUMERATOR_IN_2_{minor} GLIB_DEPRECATED_ENUMERATOR
#define {ns}_DEPRECATED_ENUMERATOR_IN_2_{minor}_FOR(f) GLIB_DEPRECATED_ENUMERATOR_FOR (f)
#define {ns}_DEPRECATED_TYPE_IN_2_{minor} GLIB_DEPRECATED_TYPE
#define {ns}_DEPRECATED_TYPE_IN_2_{minor}_FOR(f) GLIB_DEPRECATED_TYPE_FOR (f)
#else
#define {ns}_DEPRECATED_IN_2_{minor} _{ns}_EXTERN
#define {ns}_DEPRECATED_IN_2_{minor}_FOR(f) _{ns}_EXTERN
#define {ns}_DEPRECATED_MACRO_IN_2_{minor}
#define {ns}_DEPRECATED_MACRO_IN_2_{minor}_FOR(f)
#define {ns}_DEPRECATED_ENUMERATOR_IN_2_{minor}
#define {ns}_DEPRECATED_ENUMERATOR_IN_2_{minor}_FOR(f)
#define {ns}_DEPRECATED_TYPE_IN_2_{minor}
#define {ns}_DEPRECATED_TYPE_IN_2_{minor}_FOR(f)
#endif

#if GLIB_VERSION_MAX_ALLOWED < GLIB_VERSION_2_{minor}
#define {ns}_AVAILABLE_IN_2_{minor} {ns}_UNAVAILABLE (2, {minor})
#define {ns}_AVAILABLE_STATIC_INLINE_IN_2_{minor} GLIB_UNAVAILABLE_STATIC_INLINE (2, {minor})
#define {ns}_AVAILABLE_MACRO_IN_2_{minor} GLIB_UNAVAILABLE_MACRO (2, {minor})
#define {ns}_AVAILABLE_ENUMERATOR_IN_2_{minor} GLIB_UNAVAILABLE_ENUMERATOR (2, {minor})
#define {ns}_AVAILABLE_TYPE_IN_2_{minor} GLIB_UNAVAILABLE_TYPE (2, {minor})
#else
#define {ns}_AVAILABLE_IN_2_{minor} _{ns}_EXTERN
#define {ns}_AVAILABLE_STATIC_INLINE_IN_2_{minor}
#define {ns}_AVAILABLE_MACRO_IN_2_{minor}
#define {ns}_AVAILABLE_ENUMERATOR_IN_2_{minor}
#define {ns}_AVAILABLE_TYPE_IN_2_{minor}
#endif
""".format(minor = m, ns = namespace))

    native.genrule(
        name = name,
        outs = [out],
        cmd = "echo '%s' > $@" % "".join(content),
    )
