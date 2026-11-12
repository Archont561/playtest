#ifndef PLAYTEST_H
#define PLAYTEST_H

#ifndef PLAYTEST_C_VER
#error "PLAYTEST_C_VER must be defined by CMake or the build system"
#endif

#define PLAYTEST_C_STR_IMPL(x) #x
#define PLAYTEST_C_STR(x) PLAYTEST_C_STR_IMPL(x)
#define PLAYTEST_VERSION PLAYTEST_C_STR(PLAYTEST_C_VER)

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Returns the semantic version of the Playtest C library.
 */
const char *playtest_version(void);

#ifdef __cplusplus
}
#endif

#endif /* PLAYTEST_H */
