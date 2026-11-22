/* Offline process-tree fixture for job_process_meter (lane A).
 *
 *   tree root <dir>     the managed child: forks an intermediate that forks
 *                       the grandchild and exits, waits only for the
 *                       intermediate, then waits for <dir>/grandchild.done.
 *                       The grandchild stays in the root's process group,
 *                       reparented away from the tree, and is never waited.
 *   tree outside <dir>  a CPU-burning process outside every job tree.
 *   tree sleeper <dir>  a managed child that waits for <dir>/release.
 *
 * The grandchild holds HELD resident bytes, writes WRITTEN bytes to disk
 * (uncached and synced, so they are disk I/O of that process), reads them
 * back uncached, publishes HELD, WRITTEN, the bytes read and its PID, then
 * burns CPU until <dir>/release exists, touching every held page on each
 * pass so the pages stay active and resident. Files are published by
 * rename, so a reader sees either nothing or the whole content.
 */
#include <fcntl.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

#define HELD (48u << 20)
#define BLOCK (1u << 20)
#define WRITTEN (4u * BLOCK)

static char dir[900];

static void path(char *out, size_t size, const char *name) {
    snprintf(out, size, "%s/%s", dir, name);
}

static int exists(const char *name) {
    char p[1024];
    struct stat st;
    path(p, sizeof p, name);
    return stat(p, &st) == 0;
}

static void publish(const char *name, long value) {
    char tmp[1024], final[1024], hidden[128];
    snprintf(hidden, sizeof hidden, ".%s.tmp", name);
    path(tmp, sizeof tmp, hidden);
    path(final, sizeof final, name);
    FILE *file = fopen(tmp, "w");
    if (!file || fprintf(file, "%ld\n", value) < 0 || fclose(file) != 0 || rename(tmp, final) != 0) {
        _exit(70);
    }
}

static void wait_for(const char *name) {
    while (!exists(name)) {
        usleep(2000);
    }
}

static unsigned long burn_until(const char *name) {
    volatile unsigned long x = 1;
    while (!exists(name)) {
        for (int i = 0; i < 2000000; i++) {
            x = x * 6364136223846793005UL + 1442695040888963407UL;
        }
    }
    return x;
}

static int open_uncached(const char *p, int flags) {
    int fd = open(p, flags, 0600);
    if (fd < 0 || fcntl(fd, F_NOCACHE, 1) != 0) {
        _exit(72);
    }
    return fd;
}

/* One CPU-burning pass of the grandchild's loop. */
static unsigned long burn_until_pass(void) {
    volatile unsigned long x = 1;
    for (int i = 0; i < 2000000; i++) {
        x = x * 6364136223846793005UL + 1442695040888963407UL;
    }
    return x;
}

static void grandchild(void) {
    char *held = malloc(HELD);
    char *block = malloc(BLOCK);
    char p[1024];
    if (!held || !block) {
        _exit(71);
    }
    memset(held, 0x5a, HELD);
    memset(block, 0xa5, BLOCK);
    path(p, sizeof p, "output.bin");
    int fd = open_uncached(p, O_WRONLY | O_CREAT | O_TRUNC);
    for (unsigned written = 0; written < WRITTEN;) {
        ssize_t n = write(fd, block, BLOCK);
        if (n <= 0) {
            _exit(73);
        }
        written += (unsigned)n;
    }
    if (fsync(fd) != 0 || close(fd) != 0) {
        _exit(74);
    }
    fd = open_uncached(p, O_RDONLY);
    unsigned read_back = 0;
    while (read_back < WRITTEN) {
        ssize_t n = read(fd, block, BLOCK);
        if (n <= 0) {
            _exit(75);
        }
        read_back += (unsigned)n;
    }
    if (close(fd) != 0) {
        _exit(76);
    }
    publish("held", (long)HELD);
    publish("written", (long)WRITTEN);
    publish("read", (long)read_back);
    publish("grandchild.pid", (long)getpid());
    unsigned long x = 1;
    while (!exists("release")) {
        x += burn_until_pass();
        for (unsigned page = 0; page < HELD; page += 4096) {
            held[page] = (char)(held[page] + 1); /* keep the held pages live */
        }
    }
    publish("grandchild.done", (long)held[x % HELD]);
    _exit(0);
}

int main(int argc, char **argv) {
    if (argc != 3 || strlen(argv[2]) >= sizeof dir) {
        return 64;
    }
    snprintf(dir, sizeof dir, "%s", argv[2]);
    if (strcmp(argv[1], "root") == 0) {
        publish("root.pid", (long)getpid());
        pid_t middle = fork();
        if (middle < 0) {
            return 70;
        }
        if (middle == 0) {
            pid_t g = fork();
            if (g == 0) {
                grandchild();
            }
            _exit(g < 0 ? 70 : 0);
        }
        int status = 0;
        if (waitpid(middle, &status, 0) != middle || !WIFEXITED(status) || WEXITSTATUS(status) != 0) {
            return 71;
        }
        wait_for("grandchild.done");
        return 0;
    }
    if (strcmp(argv[1], "outside") == 0) {
        publish("outside.pid", (long)getpid());
        (void)burn_until("release");
        return 0;
    }
    if (strcmp(argv[1], "sleeper") == 0) {
        publish("sleeper.pid", (long)getpid());
        wait_for("release");
        return 0;
    }
    return 64;
}
