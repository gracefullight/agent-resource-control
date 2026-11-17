// Test-only clock interposition for the actual macOS arcctl process. The
// daemon and all other processes keep the system clock.
#include <errno.h>
#include <fcntl.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

static void acknowledge(const char *variable) {
    const char *path = getenv(variable);
    if (path == NULL) abort();
    int fd = open(path, O_WRONLY | O_CREAT | O_EXCL, 0600);
    if (fd >= 0) {
        if (close(fd) != 0) abort();
    } else if (errno != EEXIST) {
        abort();
    }
}

static int controlled_clock(clockid_t clock_id, struct timespec *value) {
    if (clock_id == CLOCK_REALTIME) return clock_gettime(clock_id, value);
    const char *path = getenv("ARC_STATUS_TEST_CLOCK");
    if (path == NULL) abort();
    int fd = open(path, O_RDONLY);
    if (fd < 0) abort();
    char text[8] = {0};
    ssize_t count = read(fd, text, sizeof(text) - 1);
    if (close(fd) != 0 || count <= 0) abort();
    long millis;
    if (strcmp(text, "0") == 0) {
        millis = 0;
        acknowledge("ARC_STATUS_CLOCK_INITIAL");
    } else if (strcmp(text, "250") == 0) {
        millis = 250;
        acknowledge("ARC_STATUS_CLOCK_ADVANCED");
    } else {
        abort();
    }
    value->tv_sec = 1000000;
    value->tv_nsec = millis * 1000000;
    return 0;
}

__attribute__((used)) static struct {
    const void *replacement;
    const void *original;
} clock_interpose __attribute__((section("__DATA,__interpose"))) = {
    (const void *)controlled_clock, (const void *)clock_gettime
};
