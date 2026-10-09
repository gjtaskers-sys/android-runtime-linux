#ifndef ANDROID_RUNTIME_SOCKET_PATH_H
#define ANDROID_RUNTIME_SOCKET_PATH_H

#include <stddef.h>
#include <stdio.h>
#include <stdlib.h>

static inline const char *android_runtime_display_socket_path(char *storage, size_t storage_size) {
    const char *explicit_path = getenv("ANDROID_RUNTIME_DISPLAY_SOCKET");
    if (explicit_path && *explicit_path) {
        return explicit_path;
    }

    const char *xdg = getenv("XDG_RUNTIME_DIR");
    if (xdg && *xdg) {
        if (storage && storage_size > 0) {
            (void)snprintf(storage, storage_size, "%s/android-runtime/display.sock", xdg);
        }
        return storage;
    }

    return "/tmp/android-runtime-display.sock";
}

#endif
