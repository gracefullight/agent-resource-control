// Independent native reference: getrusage SELF + waited CHILDREN, not Rust's
// timeval conversion or the outer wait4. Every owned descendant is waited for.
#include <errno.h>
#include <inttypes.h>
#include <libproc.h>
#include <math.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/resource.h>
#include <sys/sysctl.h>
#include <sys/wait.h>
#include <time.h>
#include <unistd.h>

enum { WORKERS = 2, SHORT_PER_WORKER = 4 };
struct identity { pid_t pid; uint64_t start; };
struct worker_receipt {
    struct identity identity;
    struct identity short_lived[SHORT_PER_WORKER];
    unsigned reaped;
};

static void fail(const char *operation) { perror(operation); exit(2); }
static double now(void) {
    struct timespec value;
    if (clock_gettime(CLOCK_MONOTONIC, &value)) fail("clock_gettime");
    return (double)value.tv_sec + (double)value.tv_nsec / 1e9;
}
static struct identity identity(pid_t pid) {
    struct proc_bsdinfo value = {0};
    if (proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, &value, sizeof(value)) != sizeof(value))
        fail("proc_pidinfo");
    return (struct identity){pid, value.pbi_start_tvsec * 1000000 + value.pbi_start_tvusec};
}
static void burn_until(double deadline) {
    volatile uint64_t value = 1;
    while (now() < deadline) {
        for (unsigned n = 0; n < 32768; ++n) value = value * 1664525u + 1013904223u;
    }
}
static void write_all(int fd, const void *data, size_t size) {
    const char *bytes = data;
    while (size) {
        ssize_t written = write(fd, bytes, size);
        if (written < 0 && errno == EINTR) continue;
        if (written <= 0) fail("write");
        bytes += written; size -= (size_t)written;
    }
}
static void read_all(int fd, void *data, size_t size) {
    char *bytes = data;
    while (size) {
        ssize_t received = read(fd, bytes, size);
        if (received < 0 && errno == EINTR) continue;
        if (received <= 0) fail("read");
        bytes += received; size -= (size_t)received;
    }
}
static void reap(pid_t pid) {
    int status;
    pid_t result;
    do { result = waitpid(pid, &status, 0); } while (result < 0 && errno == EINTR);
    if (result != pid || !WIFEXITED(status) || WEXITSTATUS(status) != 0) {
        fprintf(stderr, "owned child did not exit 0\n"); exit(2);
    }
}
static void worker(int receipt_fd, double deadline) {
    struct worker_receipt receipt = {0};
    receipt.identity = identity(getpid());
    for (unsigned n = 0; n < SHORT_PER_WORKER; ++n) {
        int birth[2];
        if (pipe(birth)) fail("pipe");
        pid_t child = fork();
        if (child < 0) fail("fork short child");
        if (!child) {
            close(birth[0]);
            struct identity own = identity(getpid());
            write_all(birth[1], &own, sizeof(own)); close(birth[1]);
            burn_until(now() + 0.03);
            _exit(0);
        }
        close(birth[1]);
        read_all(birth[0], &receipt.short_lived[n], sizeof(struct identity));
        close(birth[0]); reap(child); ++receipt.reaped;
    }
    burn_until(deadline);
    write_all(receipt_fd, &receipt, sizeof(receipt)); close(receipt_fd);
    _exit(0);
}
static void print_identity(struct identity value) {
    printf("{\"pid\":%d,\"start_token\":\"%" PRIu64 "\"}", value.pid, value.start);
}
static void print_timeval(struct timeval value) {
    printf("{\"seconds\":%ld,\"microseconds\":%d}", value.tv_sec, value.tv_usec);
}
int main(int argc, char **argv) {
    char *end;
    double duration = argc == 2 ? strtod(argv[1], &end) : 0;
    if (argc != 2 || *end || !isfinite(duration) || duration < 0.1 || duration > 20) return 2;
    struct identity own = identity(getpid());
    char boot[37] = {0}; size_t length = sizeof(boot);
    if (sysctlbyname("kern.bootsessionuuid", boot, &length, NULL, 0) || length != 37)
        fail("boot identity");
    double started = now(), deadline = started + duration;
    int receipts[WORKERS][2]; pid_t pids[WORKERS];
    for (unsigned n = 0; n < WORKERS; ++n) {
        if (pipe(receipts[n])) fail("pipe");
        pids[n] = fork();
        if (pids[n] < 0) fail("fork worker");
        if (!pids[n]) { close(receipts[n][0]); worker(receipts[n][1], deadline); }
        close(receipts[n][1]);
    }
    struct worker_receipt observed[WORKERS];
    for (unsigned n = 0; n < WORKERS; ++n) {
        read_all(receipts[n][0], &observed[n], sizeof(observed[n]));
        close(receipts[n][0]); reap(pids[n]);
        if (observed[n].identity.pid != pids[n] || observed[n].reaped != SHORT_PER_WORKER) return 2;
    }
    struct rusage self = {0}, children = {0};
    if (getrusage(RUSAGE_SELF, &self) || getrusage(RUSAGE_CHILDREN, &children)) fail("getrusage");
    // Reference arithmetic uses integer microseconds, independently of Rust.
    int64_t user_us = ((int64_t)self.ru_utime.tv_sec + children.ru_utime.tv_sec) * 1000000
                   + self.ru_utime.tv_usec + children.ru_utime.tv_usec;
    int64_t system_us = ((int64_t)self.ru_stime.tv_sec + children.ru_stime.tv_sec) * 1000000
                     + self.ru_stime.tv_usec + children.ru_stime.tv_usec;
    printf("{\"reference_kind\":\"getrusage_self_plus_waited_children\",\"scope\":\"native_waited_descendants\",\"root\":");
    print_identity(own);
    printf(",\"boot_id\":\"%s\",\"worker_count\":%d,\"requested_seconds\":%.3f,\"elapsed_seconds\":%.9f,\"workers\":[",
           boot, WORKERS, duration, now() - started);
    for (unsigned n = 0; n < WORKERS; ++n) {
        if (n) printf(",");
        printf("{\"identity\":"); print_identity(observed[n].identity);
        printf(",\"short_lived_reaped\":%u,\"short_lived\":[", observed[n].reaped);
        for (unsigned k = 0; k < SHORT_PER_WORKER; ++k) {
            if (k) printf(","); print_identity(observed[n].short_lived[k]);
        }
        printf("]}");
    }
    printf("],\"raw\":{\"self_user\":"); print_timeval(self.ru_utime);
    printf(",\"self_system\":"); print_timeval(self.ru_stime);
    printf(",\"children_user\":"); print_timeval(children.ru_utime);
    printf(",\"children_system\":"); print_timeval(children.ru_stime);
    printf("},\"user_microseconds\":%" PRId64 ",\"system_microseconds\":%" PRId64
           ",\"total_core_seconds\":%.9f}\n", user_us, system_us, (double)(user_us + system_us) / 1e6);
    return fflush(stdout) ? 2 : 0;
}
