#define _GNU_SOURCE 1
/*
 * Android Runtime X11 display bridge — initial reference implementation.
 *
 * The Android-side HWC sends the final SurfaceFlinger client-target DMA-BUF
 * through a Unix socket using SCM_RIGHTS. This process imports/copies that
 * buffer into an X11 image. The first implementation intentionally favors a
 * boring XShm path: it works on X11 desktops without requiring EGL/GBM import
 * support in the desktop process. A future fast path can import the DMA-BUF
 * directly into EGL and avoid the CPU copy.
 */

#include "android-display.h"
#include "frame-validate.h"
#include "dma-buf-sync.h"
#include "socket-path.h"

#include <errno.h>
#include <stdint.h>
#include <stdbool.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mman.h>
#include <poll.h>
#include <sys/socket.h>
#include <sys/stat.h>
#include <sys/types.h>
#include <sys/un.h>
#include <unistd.h>

#include <X11/Xlib.h>
#include <X11/Xutil.h>
#include <X11/keysym.h>
#include <X11/extensions/XShm.h>
#include <sys/ipc.h>
#include <sys/shm.h>

#define DRM_FORMAT_XRGB8888 0x34325258u
#define DRM_FORMAT_ARGB8888 0x34325241u
#define DRM_FORMAT_XBGR8888 0x34324258u
#define DRM_FORMAT_ABGR8888 0x34324241u

static int send_ack(int fd, uint64_t sequence, uint32_t status) {
    struct android_runtime_frame_ack ack = {0};
    memcpy(ack.magic, ANDROID_RUNTIME_DISPLAY_ACK_MAGIC, sizeof(ack.magic));
    ack.version = ANDROID_RUNTIME_DISPLAY_VERSION;
    ack.status = status;
    ack.sequence = sequence;
    return send(fd, &ack, sizeof(ack), MSG_NOSIGNAL) == (ssize_t)sizeof(ack) ? 0 : -1;
}

static int recv_frame(int fd, struct android_runtime_frame_header *hdr, int *dmafd) {
    struct msghdr msg = {0};
    struct iovec iov = {.iov_base = hdr, .iov_len = sizeof(*hdr)};
    char control[CMSG_SPACE(sizeof(int))] = {0};
    *dmafd = -1;
    msg.msg_iov = &iov;
    msg.msg_iovlen = 1;
    msg.msg_control = control;
    msg.msg_controllen = sizeof(control);

    ssize_t n = recvmsg(fd, &msg, MSG_CMSG_CLOEXEC);
    if (n == 0) return 0;
    if (n < 0) return -1;

    /* Keep exactly one passed descriptor; close any extras instead of leaking them. */
    for (struct cmsghdr *cmsg = CMSG_FIRSTHDR(&msg); cmsg; cmsg = CMSG_NXTHDR(&msg, cmsg)) {
        if (cmsg->cmsg_level != SOL_SOCKET || cmsg->cmsg_type != SCM_RIGHTS ||
            cmsg->cmsg_len < CMSG_LEN(sizeof(int))) {
            continue;
        }
        size_t payload_len = cmsg->cmsg_len - CMSG_LEN(0);
        size_t fd_count = payload_len / sizeof(int);
        int *received = (int *)CMSG_DATA(cmsg);
        for (size_t i = 0; i < fd_count; ++i) {
            if (*dmafd < 0) {
                *dmafd = received[i];
            } else {
                close(received[i]);
            }
        }
    }

    if ((msg.msg_flags & (MSG_TRUNC | MSG_CTRUNC)) != 0 ||
        (size_t)n != sizeof(*hdr) || *dmafd < 0 ||
        memcmp(hdr->magic, ANDROID_RUNTIME_DISPLAY_MAGIC, 8) != 0 ||
        hdr->version != ANDROID_RUNTIME_DISPLAY_VERSION) {
        if (*dmafd >= 0) {
            close(*dmafd);
            *dmafd = -1;
        }
        errno = EPROTO;
        return -1;
    }
    return 1;
}

static int unlink_owned_socket(const char *path, const struct stat *owned) {
    struct stat current;
    if (lstat(path, &current) < 0) return errno == ENOENT ? 0 : -1;
    if (!S_ISSOCK(current.st_mode) || current.st_dev != owned->st_dev ||
        current.st_ino != owned->st_ino) {
        errno = ESTALE;
        return -1;
    }
    return unlink(path);
}

static int remove_stale_socket(const char *path, const struct sockaddr_un *addr) {
    struct stat existing;
    if (lstat(path, &existing) < 0) {
        return errno == ENOENT ? 0 : -1;
    }
    if (!S_ISSOCK(existing.st_mode)) {
        errno = EEXIST;
        return -1;
    }

    /* Never unlink a live helper's socket. Probe existing socket paths first. */
    int probe = socket(AF_UNIX, SOCK_SEQPACKET | SOCK_CLOEXEC, 0);
    if (probe < 0) return -1;
    if (connect(probe, (const struct sockaddr *)addr, sizeof(*addr)) == 0) {
        (void)shutdown(probe, SHUT_RDWR);
        close(probe);
        errno = EADDRINUSE;
        return -1;
    }

    int connect_error = errno;
    close(probe);
    if (connect_error != ECONNREFUSED && connect_error != ENOENT) {
        errno = connect_error;
        return -1;
    }

    /* Recheck the inode before removing a socket that may have been replaced. */
    struct stat current;
    if (lstat(path, &current) < 0) {
        return errno == ENOENT ? 0 : -1;
    }
    if (!S_ISSOCK(current.st_mode) || current.st_dev != existing.st_dev ||
        current.st_ino != existing.st_ino) {
        errno = EADDRINUSE;
        return -1;
    }
    return unlink(path);
}

static int create_listener(const char *path, struct stat *owned_identity) {
    struct sockaddr_un addr = {0};
    addr.sun_family = AF_UNIX;
    size_t path_len = strlen(path);
    if (path_len == 0 || path_len >= sizeof(addr.sun_path)) {
        errno = ENAMETOOLONG;
        return -1;
    }
    memcpy(addr.sun_path, path, path_len + 1);

    if (remove_stale_socket(path, &addr) < 0) return -1;

    int fd = socket(AF_UNIX, SOCK_SEQPACKET | SOCK_CLOEXEC, 0);
    if (fd < 0) return -1;
    if (bind(fd, (struct sockaddr *)&addr, sizeof(addr)) < 0) {
        int saved_errno = errno;
        close(fd);
        errno = saved_errno;
        return -1;
    }
    struct stat bound_identity;
    if (lstat(path, &bound_identity) < 0) {
        int saved_errno = errno;
        close(fd);
        errno = saved_errno;
        return -1;
    }
    if (!S_ISSOCK(bound_identity.st_mode)) {
        close(fd);
        errno = EEXIST;
        return -1;
    }
    *owned_identity = bound_identity;
    if (listen(fd, 1) < 0) {
        int saved_errno = errno;
        close(fd);
        (void)unlink_owned_socket(path, &bound_identity);
        errno = saved_errno;
        return -1;
    }
    if (chmod(path, 0600) < 0) {
        int saved_errno = errno;
        close(fd);
        (void)unlink_owned_socket(path, &bound_identity);
        errno = saved_errno;
        return -1;
    }
    return fd;
}

static bool format_is_bgra(uint32_t fourcc) {
    return fourcc == DRM_FORMAT_XRGB8888 || fourcc == DRM_FORMAT_ARGB8888;
}

static bool format_is_rgba(uint32_t fourcc) {
    return fourcc == DRM_FORMAT_XBGR8888 || fourcc == DRM_FORMAT_ABGR8888;
}

static void copy_frame_to_ximage(XImage *image, const uint8_t *src, uint32_t width,
                                 uint32_t height, uint32_t src_stride, uint32_t fourcc) {
    const bool bgra = format_is_bgra(fourcc);
    for (uint32_t y = 0; y < height; ++y) {
        uint8_t *dst = (uint8_t *)image->data + (size_t)y * image->bytes_per_line;
        const uint8_t *row = src + (size_t)y * src_stride;
        if (bgra) {
            memcpy(dst, row, (size_t)width * 4u);
            continue;
        }

        // DRM ABGR/XBGR is RGBA/BGRA in the byte stream relative to the XRGB
        // layout expected by the default X11 32-bit TrueColor visual. For the
        // initial helper, perform the channel swap on the host CPU.
        for (uint32_t x = 0; x < width; ++x) {
            const uint8_t *p = row + (size_t)x * 4u;
            uint8_t *q = dst + (size_t)x * 4u;
            q[0] = p[2];
            q[1] = p[1];
            q[2] = p[0];
            q[3] = p[3];
        }
    }
}

static const char *default_socket_path(char *storage, size_t storage_size) {
    const char *path = android_runtime_display_socket_path(storage, storage_size);
    if (storage && path == storage) {
        char dir[sizeof(storage[0]) * 4096] = {0};
        const char *xdg = getenv("XDG_RUNTIME_DIR");
        if (xdg && *xdg) {
            (void)snprintf(dir, sizeof(dir), "%s/android-runtime", xdg);
            (void)mkdir(dir, 0700);
        }
    }
    return path;
}

static void destroy_ximage(Display *display, XImage **image, XShmSegmentInfo *shm,
                           int *image_w, int *image_h) {
    if (*image) {
        XShmDetach(display, shm);
        XSync(display, False);
        XDestroyImage(*image);
        *image = NULL;
    }
    if (shm->shmaddr && shm->shmaddr != (char *)-1) {
        (void)shmdt(shm->shmaddr);
        shm->shmaddr = NULL;
    }
    if (shm->shmid >= 0) {
        (void)shmctl(shm->shmid, IPC_RMID, NULL);
        shm->shmid = -1;
    }
    *image_w = 0;
    *image_h = 0;
}

int main(int argc, char **argv) {
    if (argc > 2) {
        fprintf(stderr, "usage: %s [unix-socket-path]\n", argv[0]);
        return 2;
    }

    char default_path[4096] = {0};
    const char *socket_path = argc > 1 ? argv[1] : default_socket_path(default_path, sizeof(default_path));
    Display *display = XOpenDisplay(NULL);
    if (!display) {
        fprintf(stderr, "XOpenDisplay failed; is DISPLAY set?\n");
        return 1;
    }
    if (!XShmQueryExtension(display)) {
        fprintf(stderr, "display bridge: X server does not support MIT-SHM\n");
        XCloseDisplay(display);
        return 1;
    }
    int shm_event_base = XShmGetEventBase(display);
    if (shm_event_base < 0) {
        fprintf(stderr, "display bridge: could not determine MIT-SHM event base\n");
        XCloseDisplay(display);
        return 1;
    }
    const int shm_completion_event = shm_event_base + ShmCompletion;

    struct stat socket_identity;
    int server = create_listener(socket_path, &socket_identity);
    if (server < 0) {
        perror("create_listener");
        XCloseDisplay(display);
        return 1;
    }

    int screen = DefaultScreen(display);
    Window window = XCreateSimpleWindow(display, RootWindow(display, screen), 0, 0, 720, 1280, 0,
                                        BlackPixel(display, screen), BlackPixel(display, screen));
    XStoreName(display, window, "Android Runtime");
    XSelectInput(display, window, ExposureMask | KeyPressMask | StructureNotifyMask);
    Atom wm_protocols = XInternAtom(display, "WM_PROTOCOLS", False);
    Atom wm_delete_window = XInternAtom(display, "WM_DELETE_WINDOW", False);
    if (!XSetWMProtocols(display, window, &wm_delete_window, 1)) {
        fprintf(stderr, "display bridge: could not register WM_DELETE_WINDOW\n");
    }
    XMapWindow(display, window);
    XFlush(display);

    XShmSegmentInfo shm = {0};
    shm.shmid = -1;
    XImage *image = NULL;
    int image_w = 0, image_h = 0;
    int client = -1;
    bool shm_transfer_pending = false;
    bool frame_ack_pending = false;
    bool repaint_needed = false;
    uint64_t pending_sequence = 0;
    bool running = true;
    int exit_status = 0;
    int x_fd = ConnectionNumber(display);

    /* Poll X11 and the frame channel together so the window remains responsive
       while the producer is idle or connected but not currently sending frames. */
    while (running) {
        struct pollfd fds[3];
        nfds_t count = 0;
        int server_index = -1, x_index = -1, client_index = -1;
        if (client < 0) {
            server_index = (int)count;
            fds[count++] = (struct pollfd){.fd = server, .events = POLLIN};
        }
        x_index = (int)count;
        fds[count++] = (struct pollfd){.fd = x_fd, .events = POLLIN};
        /* Keep the single shared XShm image immutable until every server read
           has completed. The producer also waits while a protocol frame is pending. */
        if (client >= 0 && !frame_ack_pending && !shm_transfer_pending) {
            client_index = (int)count;
            fds[count++] = (struct pollfd){.fd = client, .events = POLLIN};
        }

        int poll_result = poll(fds, count, -1);
        if (poll_result < 0) {
            if (errno == EINTR) continue;
            perror("poll");
            exit_status = 1;
            break;
        }

        if (fds[x_index].revents & (POLLERR | POLLHUP | POLLNVAL)) {
            fprintf(stderr, "display bridge: X11 connection closed\n");
            exit_status = 1;
            break;
        }
        if (fds[x_index].revents & POLLIN) {
            while (XPending(display) > 0) {
                XEvent event;
                XNextEvent(display, &event);
                if (event.type == shm_completion_event) {
                    XShmCompletionEvent *completion = (XShmCompletionEvent *)&event;
                    if (shm_transfer_pending && completion->drawable == window &&
                        completion->shmseg == shm.shmseg) {
                        shm_transfer_pending = false;
                        if (repaint_needed && image) {
                            repaint_needed = false;
                            if (XShmPutImage(display, window, DefaultGC(display, screen), image,
                                             0, 0, 0, 0, (unsigned int)image_w,
                                             (unsigned int)image_h, True)) {
                                shm_transfer_pending = true;
                                XFlush(display);
                                continue;
                            }
                            fprintf(stderr, "display bridge: deferred expose repaint failed\n");
                        }
                        if (frame_ack_pending) {
                            frame_ack_pending = false;
                            if (client < 0 || send_ack(client, pending_sequence, 0) < 0) {
                                if (client >= 0) close(client);
                                client = -1;
                            }
                        }
                    }
                    continue;
                }
                if (event.type == ClientMessage && event.xclient.window == window &&
                    event.xclient.message_type == wm_protocols &&
                    (Atom)event.xclient.data.l[0] == wm_delete_window) {
                    running = false;
                    break;
                }
                if (event.type == KeyPress) {
                    KeySym key = XLookupKeysym(&event.xkey, 0);
                    if (key == XK_Escape || key == XK_q) {
                        running = false;
                        break;
                    }
                }
                if (event.type == Expose && event.xexpose.count == 0 && image) {
                    if (shm_transfer_pending) {
                        repaint_needed = true;
                    } else if (XShmPutImage(display, window, DefaultGC(display, screen), image,
                                            0, 0, 0, 0, (unsigned int)image_w,
                                            (unsigned int)image_h, True)) {
                        shm_transfer_pending = true;
                        XFlush(display);
                    } else {
                        fprintf(stderr, "display bridge: expose repaint failed\n");
                    }
                }
            }
        }
        if (!running) break;

        if (server_index >= 0 && (fds[server_index].revents & POLLIN)) {
            client = accept4(server, NULL, NULL, SOCK_CLOEXEC);
            if (client < 0 && errno != EINTR) {
                perror("accept4");
                exit_status = 1;
                break;
            }
            continue;
        }

        if (client_index >= 0 && fds[client_index].revents & (POLLIN | POLLHUP | POLLERR | POLLNVAL)) {
            struct android_runtime_frame_header hdr;
            int dmafd = -1;
            int result = -1;
            if (fds[client_index].revents & POLLIN) {
                result = recv_frame(client, &hdr, &dmafd);
            } else {
                errno = ECONNRESET;
            }
            if (result <= 0) {
                if (result < 0 && errno != ECONNRESET && errno != EPIPE) {
                    fprintf(stderr, "display bridge: invalid frame message: %s\n", strerror(errno));
                }
                close(client);
                client = -1;
                continue;
            }

            const uint64_t modifier = ((uint64_t)hdr.modifier_hi << 32) | hdr.modifier_lo;
            const bool supported_format = format_is_bgra(hdr.drm_fourcc) || format_is_rgba(hdr.drm_fourcc);
            size_t map_size = 0;
            if (!android_runtime_valid_frame_geometry(&hdr, &map_size) || modifier != 0 || !supported_format) {
                fprintf(stderr, "display bridge: invalid or unsupported frame metadata\n");
                (void)send_ack(client, hdr.sequence, 1);
                if (dmafd >= 0) close(dmafd);
                continue;
            }
            void *mapped = mmap(NULL, map_size, PROT_READ, MAP_SHARED, dmafd, 0);
            if (mapped == MAP_FAILED) {
                (void)send_ack(client, hdr.sequence, 2);
                close(dmafd);
                continue;
            }

            if (!image || image_w != (int)hdr.width || image_h != (int)hdr.height) {
                destroy_ximage(display, &image, &shm, &image_w, &image_h);
                image = XShmCreateImage(display, DefaultVisual(display, screen), 24, ZPixmap, NULL, &shm,
                                        hdr.width, hdr.height);
                if (!image) {
                    munmap(mapped, map_size);
                    (void)send_ack(client, hdr.sequence, 3);
                    close(dmafd);
                    continue;
                }
                size_t bytes = (size_t)image->bytes_per_line * (size_t)image->height;
                shm.shmid = shmget(IPC_PRIVATE, bytes, IPC_CREAT | 0600);
                if (shm.shmid < 0) {
                    XDestroyImage(image);
                    image = NULL;
                    munmap(mapped, map_size);
                    (void)send_ack(client, hdr.sequence, 3);
                    close(dmafd);
                    continue;
                }
                shm.shmaddr = (char *)shmat(shm.shmid, NULL, 0);
                if (shm.shmaddr == (char *)-1) {
                    shm.shmaddr = NULL;
                    shmctl(shm.shmid, IPC_RMID, NULL);
                    shm.shmid = -1;
                    XDestroyImage(image);
                    image = NULL;
                    munmap(mapped, map_size);
                    (void)send_ack(client, hdr.sequence, 3);
                    close(dmafd);
                    continue;
                }
                shm.readOnly = False;
                image->data = shm.shmaddr;
                XShmAttach(display, &shm);
                XSync(display, False);
                (void)shmctl(shm.shmid, IPC_RMID, NULL);
                image_w = (int)hdr.width;
                image_h = (int)hdr.height;
                XResizeWindow(display, window, hdr.width, hdr.height);
            }

            const bool needs_dma_buf_sync =
                (hdr.flags & ANDROID_RUNTIME_FRAME_FLAG_DMA_BUF_CPU_SYNC) != 0;
            if (needs_dma_buf_sync &&
                android_runtime_dma_buf_sync(dmafd, DMA_BUF_SYNC_START | DMA_BUF_SYNC_READ) < 0) {
                fprintf(stderr, "display bridge: DMA-BUF CPU read START sync failed: %s\n",
                        strerror(errno));
                (void)send_ack(client, hdr.sequence, 4);
                munmap(mapped, map_size);
                close(dmafd);
                continue;
            }

            copy_frame_to_ximage(image, mapped, hdr.width, hdr.height, hdr.stride, hdr.drm_fourcc);

            if (needs_dma_buf_sync &&
                android_runtime_dma_buf_sync(dmafd, DMA_BUF_SYNC_END | DMA_BUF_SYNC_READ) < 0) {
                fprintf(stderr, "display bridge: DMA-BUF CPU read END sync failed: %s\n",
                        strerror(errno));
                (void)send_ack(client, hdr.sequence, 4);
                munmap(mapped, map_size);
                close(dmafd);
                continue;
            }

            if (!XShmPutImage(display, window, DefaultGC(display, screen), image, 0, 0, 0, 0,
                              hdr.width, hdr.height, True)) {
                (void)send_ack(client, hdr.sequence, 3);
                munmap(mapped, map_size);
                close(dmafd);
                continue;
            }
            shm_transfer_pending = true;
            frame_ack_pending = true;
            pending_sequence = hdr.sequence;
            XFlush(display);

            munmap(mapped, map_size);
            close(dmafd);
            /* ACK only after ShmCompletion: the X server is done reading image->data,
               so the next frame cannot overwrite shared pixels still in use. */
        }
    }

    if (client >= 0) close(client);
    destroy_ximage(display, &image, &shm, &image_w, &image_h);
    XDestroyWindow(display, window);
    XFlush(display);
    close(server);
    if (unlink_owned_socket(socket_path, &socket_identity) < 0 && errno != ENOENT) {
        fprintf(stderr, "display bridge: socket path changed; leaving replacement untouched\n");
    }
    XCloseDisplay(display);
    return exit_status;
}
