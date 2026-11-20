/* Inject a monotonic advance into the CLI after the READY checkpoint.
 * The native target and daemon continue using their own clocks. */
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <limits.h>

static char control_path[PATH_MAX];
static int target_cli;

__attribute__((constructor)) static void configure_ready_clock(void) {
    const char *control = getenv("ARC_TEST_READY_CLOCK_CONTROL");
    const char *program = getprogname();
    if (control == NULL || program == NULL || strcmp(program, "arcctl") != 0) {
        return;
    }
    size_t size = strlen(control) + 1;
    if (size <= sizeof(control_path)) {
        memcpy(control_path, control, size);
        target_cli = 1;
    }
}

static int ready_clock_gettime(clockid_t clock, struct timespec *value) {
    int result = clock_gettime(clock, value);
    /* access is async-signal-safe when the launch gate reads the clock after
     * fork. Configuration and string handling happened before CLI startup. */
    if (result == 0 && clock == CLOCK_UPTIME_RAW && target_cli &&
        access(control_path, F_OK) == 0) {
        value->tv_sec += 2;
    }
    return result;
}

/* dyld leaves this library's own call to clock_gettime bound to the original. */
__attribute__((used)) static struct {
    const void *replacement;
    const void *original;
} interpose_clock __attribute__((section("__DATA,__interpose"))) = {
    (const void *)ready_clock_gettime,
    (const void *)clock_gettime,
};
