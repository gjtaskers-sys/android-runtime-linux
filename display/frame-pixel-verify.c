#define _POSIX_C_SOURCE 200809L
#include <X11/Xlib.h>
#include <X11/Xutil.h>

#include <stdbool.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#define SMOKE_WIDTH 720u
#define SMOKE_HEIGHT 1280u

static Window find_named_window(Display *display, Window current, const char *wanted,
                                unsigned depth) {
    if (depth > 32) return None;

    char *name = NULL;
    if (XFetchName(display, current, &name) && name) {
        bool matches = strcmp(name, wanted) == 0;
        XFree(name);
        if (matches) return current;
    }

    Window root = None, parent = None, *children = NULL;
    unsigned int child_count = 0;
    if (!XQueryTree(display, current, &root, &parent, &children, &child_count)) return None;
    Window found = None;
    for (unsigned int i = 0; i < child_count && found == None; ++i) {
        found = find_named_window(display, children[i], wanted, depth + 1);
    }
    if (children) XFree(children);
    return found;
}

static unsigned channel_8(unsigned long pixel, unsigned long mask) {
    if (mask == 0) return 0;
    unsigned int shift = 0;
    const unsigned int bits = (unsigned int)(sizeof(mask) * 8u);
    while (shift < bits && ((mask >> shift) & 1ul) == 0) ++shift;
    if (shift == bits) return 0;
    unsigned long max_value = mask >> shift;
    unsigned long value = (pixel & mask) >> shift;
    return (unsigned)((value * 255ul + max_value / 2ul) / max_value);
}

static unsigned scaled(unsigned value, unsigned level) {
    return (value * level + 127u) / 255u;
}

static void expected_rgb(unsigned x, unsigned y, unsigned frame_index,
                         unsigned rgb[3]) {
    unsigned phase = frame_index % 16u;
    if (phase > 8u) phase = 16u - phase;
    const unsigned level = 191u + phase * 8u;
    const unsigned red = (x * 255u) / (SMOKE_WIDTH - 1u);
    const unsigned green = (y * 255u) / (SMOKE_HEIGHT - 1u);
    const unsigned blue = ((x + y) * 255u) / (SMOKE_WIDTH + SMOKE_HEIGHT - 2u);
    rgb[0] = scaled(red, level);
    rgb[1] = scaled(green, level);
    rgb[2] = scaled(blue, level);
}

static int verify_pixel(Display *display, Window window, Visual *visual,
                        unsigned x, unsigned y, unsigned frame_index) {
    XImage *image = XGetImage(display, window, (int)x, (int)y, 1, 1,
                              AllPlanes, ZPixmap);
    if (!image) {
        fprintf(stderr, "could not read X11 pixel at (%u,%u)\n", x, y);
        return -1;
    }
    unsigned long pixel = XGetPixel(image, 0, 0);
    XDestroyImage(image);

    unsigned actual[3] = {
        channel_8(pixel, visual->red_mask),
        channel_8(pixel, visual->green_mask),
        channel_8(pixel, visual->blue_mask),
    };
    unsigned expected[3];
    expected_rgb(x, y, frame_index, expected);

    for (unsigned i = 0; i < 3; ++i) {
        unsigned delta = actual[i] > expected[i] ? actual[i] - expected[i]
                                                 : expected[i] - actual[i];
        if (delta > 4u) {
            static const char *const names[3] = {"red", "green", "blue"};
            fprintf(stderr,
                    "X11 pixel mismatch at (%u,%u): %s expected %u, got %u "
                    "(RGB=%u,%u,%u)\n",
                    x, y, names[i], expected[i], actual[i],
                    actual[0], actual[1], actual[2]);
            return -1;
        }
    }
    return 0;
}

int main(int argc, char **argv) {
    if (argc != 3) {
        fprintf(stderr, "usage: %s window-title frame-count(1..1000)\n", argv[0]);
        return 2;
    }
    char *end = NULL;
    unsigned long frame_count = strtoul(argv[2], &end, 10);
    if (!end || end == argv[2] || *end != '\0' || frame_count == 0 || frame_count > 1000) {
        fprintf(stderr, "frame-count must be an integer from 1 to 1000\n");
        return 2;
    }

    Display *display = XOpenDisplay(NULL);
    if (!display) {
        fprintf(stderr, "XOpenDisplay failed; is DISPLAY set?\n");
        return 1;
    }
    Window window = find_named_window(display, DefaultRootWindow(display), argv[1], 0);
    if (window == None) {
        fprintf(stderr, "could not find X11 window titled '%s'\n", argv[1]);
        XCloseDisplay(display);
        return 1;
    }

    XWindowAttributes attrs;
    if (!XGetWindowAttributes(display, window, &attrs) ||
        attrs.width < (int)SMOKE_WIDTH || attrs.height < (int)SMOKE_HEIGHT) {
        fprintf(stderr, "X11 window dimensions are smaller than the smoke frame\n");
        XCloseDisplay(display);
        return 1;
    }
    XRaiseWindow(display, window);
    XSync(display, False);

    int screen = DefaultScreen(display);
    int screen_width = DisplayWidth(display, screen);
    int screen_height = DisplayHeight(display, screen);
    if (screen_width < 80 || screen_height < 160) {
        fprintf(stderr, "X11 screen is too small for color readback validation\n");
        XCloseDisplay(display);
        return 1;
    }
    unsigned x_sample = SMOKE_WIDTH - 1u;
    unsigned y_sample = SMOKE_HEIGHT / 3u;
    if (screen_width < (int)SMOKE_WIDTH + 20) {
        x_sample = (unsigned)(screen_width - 40);
    }
    if (screen_height < (int)y_sample + 80) {
        y_sample = (unsigned)(screen_height - 80);
    }

    const unsigned last_frame = (unsigned)(frame_count - 1ul);
    const unsigned points[][2] = {
        {0, 0},
        {x_sample, 0},
        {0, y_sample},
        {x_sample, y_sample},
        {x_sample / 2u, y_sample / 2u},
    };
    for (size_t i = 0; i < sizeof(points) / sizeof(points[0]); ++i) {
        if (verify_pixel(display, window, attrs.visual,
                         points[i][0], points[i][1], last_frame) < 0) {
            XCloseDisplay(display);
            return 1;
        }
    }

    printf("X11 pixel readback passed: smooth RGB gradient and channel orientation verified\n");
    XCloseDisplay(display);
    return 0;
}
