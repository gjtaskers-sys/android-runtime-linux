#define _GNU_SOURCE 1
#include "android-display.h"
#include "socket-path.h"

#include <errno.h>
#include <fcntl.h>
#include <poll.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <sys/syscall.h>
#include <sys/socket.h>
#include <sys/types.h>
#include <sys/un.h>
#include <unistd.h>

#ifndef MFD_CLOEXEC
#define MFD_CLOEXEC 0x0001U
#endif

static int memfd_create_compat(const char *name) {
    return syscall(SYS_memfd_create, name, MFD_CLOEXEC);
}

static int parse_frame_count(const char *text, unsigned long *out) {
    if (text == NULL || *text == '\0') return -1;
    errno = 0;
    char *end = NULL;
    unsigned long value = strtoul(text, &end, 10);
    if (errno != 0 || end == text || *end != '\0' || value == 0 || value > 1000) return -1;
    *out = value;
    return 0;
}

int main(int argc, char **argv) {
    if (argc > 3) {
        fprintf(stderr, "usage: %s [unix-socket-path] [frame-count 1..1000]\n", argv[0]);
        return 2;
    }
    char default_path[4096] = {0};
    const char *path = argc > 1 ? argv[1] : android_runtime_display_socket_path(default_path, sizeof(default_path));
    unsigned long frame_count = 8;
    if (argc > 2 && parse_frame_count(argv[2], &frame_count) < 0) {
        fprintf(stderr, "frame-count must be an integer from 1 to 1000\n");
        return 2;
    }
    const uint32_t width = 720;
    const uint32_t height = 1280;
    const uint32_t stride = width * 4;
    const size_t size = (size_t)stride * height;

    int memfd = memfd_create_compat("android-runtime-frame");
    if (memfd < 0 || ftruncate(memfd, (off_t)size) < 0) {
        perror("memfd/ftruncate");
        return 1;
    }
    uint32_t *pixels = mmap(NULL, size, PROT_READ | PROT_WRITE, MAP_SHARED, memfd, 0);
    if (pixels == MAP_FAILED) {
        perror("mmap");
        close(memfd);
        return 1;
    }

    int sock = socket(AF_UNIX, SOCK_SEQPACKET | SOCK_CLOEXEC, 0);
    if (sock < 0) {
        perror("socket");
        munmap(pixels, size);
        close(memfd);
        return 1;
    }
    struct sockaddr_un addr = {0};
    addr.sun_family = AF_UNIX;
    size_t path_len = strlen(path);
    if (path_len >= sizeof(addr.sun_path)) {
        fprintf(stderr, "display socket path too long\n");
        close(sock);
        munmap(pixels, size);
        close(memfd);
        return 1;
    }
    memcpy(addr.sun_path, path, path_len + 1);
    if (connect(sock, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
        perror("connect");
        close(sock);
        munmap(pixels, size);
        close(memfd);
        return 1;
    }

    struct android_runtime_frame_header hdr = {0};
    memcpy(hdr.magic, ANDROID_RUNTIME_DISPLAY_MAGIC, sizeof(hdr.magic));
    hdr.version = ANDROID_RUNTIME_DISPLAY_VERSION;
    hdr.width = width;
    hdr.height = height;
    hdr.stride = stride;
    hdr.drm_fourcc = 0x34325258u; // DRM_FORMAT_XRGB8888
    hdr.flags = ANDROID_RUNTIME_FRAME_FLAG_CLIENT_TARGET;

    for (unsigned long frame = 0; frame < frame_count; ++frame) {
        /* Change the source pixels each iteration to exercise buffer reuse. */
        /* Use a continuous two-dimensional gradient. Change only brightness
           between frames; do not add independent RGB offsets that wrap at 255
           and create abrupt color bands that look like rendering corruption. */
        unsigned phase = (unsigned)(frame % 16u);
        if (phase > 8u) phase = 16u - phase;
        const unsigned level = 191u + phase * 8u;
        for (uint32_t y = 0; y < height; ++y) {
            for (uint32_t x = 0; x < width; ++x) {
                unsigned base_r = (x * 255u) / (width - 1u);
                unsigned base_g = (y * 255u) / (height - 1u);
                unsigned base_b = ((x + y) * 255u) / (width + height - 2u);
                uint8_t r = (uint8_t)((base_r * level + 127u) / 255u);
                uint8_t g = (uint8_t)((base_g * level + 127u) / 255u);
                uint8_t b = (uint8_t)((base_b * level + 127u) / 255u);
                pixels[(size_t)y * width + x] = 0xff000000u | ((uint32_t)r << 16) |
                                                ((uint32_t)g << 8) | b;
            }
        }

        hdr.sequence = frame + 1;
        struct iovec iov = {&hdr, sizeof(hdr)};
        char control[CMSG_SPACE(sizeof(int))] = {0};
        struct msghdr msg = {0};
        msg.msg_iov = &iov;
        msg.msg_iovlen = 1;
        msg.msg_control = control;
        msg.msg_controllen = sizeof(control);
        struct cmsghdr *cmsg = CMSG_FIRSTHDR(&msg);
        cmsg->cmsg_level = SOL_SOCKET;
        cmsg->cmsg_type = SCM_RIGHTS;
        cmsg->cmsg_len = CMSG_LEN(sizeof(int));
        memcpy(CMSG_DATA(cmsg), &memfd, sizeof(int));

        if (sendmsg(sock, &msg, MSG_NOSIGNAL) != (ssize_t)sizeof(hdr)) {
            perror("sendmsg");
            close(sock);
            munmap(pixels, size);
            close(memfd);
            return 1;
        }

        struct pollfd ack_poll = {.fd = sock, .events = POLLIN};
        int poll_result;
        do {
            poll_result = poll(&ack_poll, 1, 5000);
        } while (poll_result < 0 && errno == EINTR);
        if (poll_result <= 0 || !(ack_poll.revents & POLLIN)) {
            fprintf(stderr, "timed out waiting for frame ACK sequence %llu\n",
                    (unsigned long long)hdr.sequence);
            close(sock);
            munmap(pixels, size);
            close(memfd);
            return 1;
        }

        struct android_runtime_frame_ack ack = {0};
        ssize_t n;
        do {
            n = recv(sock, &ack, sizeof(ack), MSG_WAITALL);
        } while (n < 0 && errno == EINTR);
        if (n != (ssize_t)sizeof(ack) ||
            memcmp(ack.magic, ANDROID_RUNTIME_DISPLAY_ACK_MAGIC, sizeof(ack.magic)) != 0 ||
            ack.version != ANDROID_RUNTIME_DISPLAY_VERSION || ack.status != 0 ||
            ack.sequence != hdr.sequence) {
            fprintf(stderr, "invalid frame acknowledgement for sequence %llu (status %u)\n",
                    (unsigned long long)hdr.sequence, ack.status);
            close(sock);
            munmap(pixels, size);
            close(memfd);
            return 1;
        }
    }

    printf("%lu changing frames sent and acknowledged by %s\n", frame_count, path);
    close(sock);
    munmap(pixels, size);
    close(memfd);
    return 0;
}
