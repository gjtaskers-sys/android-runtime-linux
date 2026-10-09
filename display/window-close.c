#define _POSIX_C_SOURCE 200809L
#include <X11/Xlib.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

static Window find_named_client(Display *display, Window current, const char *wanted,
                                Atom wm_delete) {
    char *name = NULL;
    if (XFetchName(display, current, &name) && name) {
        bool matches = strcmp(name, wanted) == 0;
        XFree(name);
        if (matches) {
            Atom *protocols = NULL;
            int protocol_count = 0;
            bool supports_delete = false;
            if (XGetWMProtocols(display, current, &protocols, &protocol_count)) {
                for (int i = 0; i < protocol_count; ++i) {
                    if (protocols[i] == wm_delete) supports_delete = true;
                }
                XFree(protocols);
            }
            if (supports_delete) return current;
        }
    }

    Window root = None, parent = None, *children = NULL;
    unsigned int child_count = 0;
    if (!XQueryTree(display, current, &root, &parent, &children, &child_count)) return None;
    Window found = None;
    for (unsigned int i = 0; i < child_count && found == None; ++i) {
        found = find_named_client(display, children[i], wanted, wm_delete);
    }
    if (children) XFree(children);
    return found;
}

int main(int argc, char **argv) {
    if (argc != 2) {
        fprintf(stderr, "usage: %s window-title\n", argv[0]);
        return 2;
    }
    Display *display = XOpenDisplay(NULL);
    if (!display) {
        fprintf(stderr, "XOpenDisplay failed; is DISPLAY set?\n");
        return 1;
    }
    Atom wm_delete = XInternAtom(display, "WM_DELETE_WINDOW", False);
    Window window = find_named_client(display, DefaultRootWindow(display), argv[1], wm_delete);
    if (window == None) {
        fprintf(stderr, "could not find a window titled '%s' that supports WM_DELETE_WINDOW\n", argv[1]);
        XCloseDisplay(display);
        return 1;
    }

    XEvent event = {0};
    event.xclient.type = ClientMessage;
    event.xclient.window = window;
    event.xclient.message_type = XInternAtom(display, "WM_PROTOCOLS", False);
    event.xclient.format = 32;
    event.xclient.data.l[0] = (long)wm_delete;
    event.xclient.data.l[1] = (long)CurrentTime;
    Status sent = XSendEvent(display, window, False, NoEventMask, &event);
    XFlush(display);
    XSync(display, False);
    XCloseDisplay(display);
    if (!sent) {
        fprintf(stderr, "failed to send WM_DELETE_WINDOW to '%s'\n", argv[1]);
        return 1;
    }
    return 0;
}
