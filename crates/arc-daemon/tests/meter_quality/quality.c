/* Offline Meter-quality fixture for meter_quality (lane B).
 *
 *   quality footprint <dir>  in its working directory: writes BIG bytes to
 *                            `big.bin` (uncached, synced, so they are disk
 *                            I/O of this process), deletes it, then writes
 *                            and keeps LEFT bytes in `left.bin`; publishes
 *                            its PID and the bytes written, waits for
 *                            <dir>/release, then burns CPU it never lets a
 *                            sample see before exiting.
 *   quality hold <dir>       publishes its PID, burns a little CPU until
 *                            <dir>/release exists, then exits.
 *   quality spike <dir>      forks a child that writes SPIKE bytes (synced)
 *                            to `spike.bin` in its working directory,
 *                            publishes its PID and exits; reaps it, deletes
 *                            the file, publishes its own PID and waits for
 *                            <dir>/release.
 *   quality quick            exits at once.
 *
 * Files in <dir> are published by rename, so a reader sees either nothing
 * or the whole content.
 */
#include <fcntl.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/resource.h>
#include <sys/stat.h>
#include <sys/wait.h>
#include <unistd.h>

#define BLOCK (1u << 20)
#define BIG (32u * BLOCK)
#define LEFT (1u * BLOCK)
#define SPIKE (4u * BLOCK)

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

static void write_synced(const char *name, unsigned bytes, char *block) {
    int fd = open(name, O_WRONLY | O_CREAT | O_TRUNC, 0600);
    if (fd < 0) {
        _exit(71);
    }
#ifdef F_NOCACHE
    if (fcntl(fd, F_NOCACHE, 1) != 0) {
        _exit(72);
    }
#endif
    for (unsigned written = 0; written < bytes;) {
        ssize_t n = write(fd, block, BLOCK);
        if (n <= 0) {
            _exit(73);
        }
        written += (unsigned)n;
    }
    if (fsync(fd) != 0 || close(fd) != 0) {
        _exit(74);
    }
}

static double cpu_seconds(void) {
    struct rusage usage;
    getrusage(RUSAGE_SELF, &usage);
    return (double)(usage.ru_utime.tv_sec + usage.ru_stime.tv_sec) +
           (double)(usage.ru_utime.tv_usec + usage.ru_stime.tv_usec) / 1e6;
}

/* Burn `seconds` of this process's own CPU time. */
static unsigned long burn(double seconds) {
    volatile unsigned long x = 1;
    double until = cpu_seconds() + seconds;
    while (cpu_seconds() < until) {
        for (int i = 0; i < 200000; i++) {
            x = x * 6364136223846793005UL + 1442695040888963407UL;
        }
    }
    return x;
}

static void wait_for(const char *name) {
    while (!exists(name)) {
        usleep(2000);
    }
}

int main(int argc, char **argv) {
    const char *mode = argc > 1 ? argv[1] : NULL;
    const char *directory = argc == 3 ? argv[2] : NULL;
    /* A copy named cargo takes the real classified `cargo build` route. */
    if (argc == 2 && mode && strcmp(mode, "build") == 0) {
        mode = getenv("ARC_QUALITY_MODE");
        directory = getenv("ARC_QUALITY_CONTROL");
    }
    if (argc == 2 && mode && strcmp(mode, "quick") == 0) {
        return 0;
    }
    if (!mode || !directory || strlen(directory) >= sizeof dir) {
        return 64;
    }
    snprintf(dir, sizeof dir, "%s", directory);
    if (strcmp(mode, "footprint") == 0) {
        char *block = malloc(BLOCK);
        if (!block) {
            return 71;
        }
        memset(block, 0xa5, BLOCK);
        write_synced("big.bin", BIG, block);
        if (unlink("big.bin") != 0) {
            return 75;
        }
        write_synced("left.bin", LEFT, block);
        publish("written", (long)(BIG + LEFT));
        publish("left", (long)LEFT);
        publish("footprint.pid", (long)getpid());
        wait_for("release");
        /* CPU after the last sample: the waited usage exceeds the sampled root. */
        return burn(0.05) == 0 ? 76 : 0;
    }
    if (strcmp(mode, "spike") == 0) {
        pid_t child = fork();
        if (child < 0) {
            return 77;
        }
        if (child == 0) {
            char *block = malloc(BLOCK);
            if (!block) {
                _exit(71);
            }
            memset(block, 0x5a, BLOCK);
            write_synced("spike.bin", SPIKE, block);
            publish("spike-child.pid", (long)getpid());
            _exit(0);
        }
        int status = 0;
        if (waitpid(child, &status, 0) != child || !WIFEXITED(status) || WEXITSTATUS(status) != 0) {
            return 78;
        }
        if (unlink("spike.bin") != 0) {
            return 75;
        }
        publish("spike.pid", (long)getpid());
        wait_for("release");
        return 0;
    }
    if (strcmp(mode, "cpu-miss") == 0) {
        publish("cpu-root.pid", (long)getpid());
        wait_for("fork");
        /* No wait: the kernel reaps the short-lived child, so wait4 of the
         * root cannot include an intermediate process's waited CPU. */
        if (signal(SIGCHLD, SIG_IGN) == SIG_ERR) return 79;
        pid_t child = fork();
        if (child < 0) return 77;
        if (child == 0) {
            publish("cpu-child.pid", (long)getpid());
            wait_for("burn");
            (void)burn(0.025);
            publish("cpu-child-us", (long)(cpu_seconds() * 1000000.0));
            _exit(0);
        }
        wait_for("cpu-child-us");
        publish("cpu-ready", 1);
        /* Stop before the final sample, so exit adds no CPU after it. */
        raise(SIGSTOP);
        return 0;
    }
    if (strcmp(mode, "mutate") == 0) {
        char *block = malloc(BLOCK);
        if (!block) return 71;
        memset(block, 0x5a, BLOCK);
        write_synced("successor.bin", BLOCK, block);
        publish("mutate.pid", (long)getpid());
        wait_for("release");
        return 0;
    }
    if (strcmp(mode, "hold") == 0) {
        publish("hold.pid", (long)getpid());
        unsigned long x = 1;
        while (!exists("release")) {
            x += burn(0.002);
            usleep(5000);
        }
        return x == 0 ? 76 : 0;
    }
    return 64;
}
