/* Offline CPU-burning fixture for process_attribution (lane A).
 *
 *   cargo build                    the managed root, run by the installed
 *                                  hook's returned command; directory from
 *                                  ARC_ATTRIBUTION_DIR. Forks a managed
 *                                  descendant; both burn until <dir>/release,
 *                                  then the root waits for the descendant.
 *   detach <exe> <mode> <dir> [x]  start <exe> <mode> <dir> [x] in a new
 *                                  session whose parent exits at once, so
 *                                  the process is reparented outside every
 *                                  ancestry of the test; standard streams
 *                                  are /dev/null.
 *   agent <dir> <burner-exe>       run under an agent's executable name:
 *                                  starts <burner-exe> burn agent-burner
 *                                  <dir> and waits for it.
 *   burn <role> <dir>              publish <role>.pid, burn until release.
 *   respawn <dir> <burner-exe>     publish respawner.pid; start
 *                                  <burner-exe> sleep-observe respawn-1 <dir>
 *                                  (almost no CPU), whose PID is the target.
 *                                  WORKERS forkers advance the kernel's
 *                                  sequential PID allocator once around its
 *                                  range to just below the target, then
 *                                  publish cycled. respawn-1 exits on its
 *                                  next scan line; the forkers continue and
 *                                  the child that receives the target PID
 *                                  execs <burner-exe> burn respawn-2 <dir>.
 *                                  Publishes reuse = 1 when it did, else 0:
 *                                  one PID, two process lifetimes, normally
 *                                  in consecutive scans.
 *   sleep-observe <role> <dir>     publish <role>.pid; record its scan lines
 *                                  without burning; exit at the first line
 *                                  after <dir>/cycled or at release.
 *
 * Files are published by rename, so a reader sees nothing or the whole value.
 *
 * Ownership: every mode first joins the test's fixture set. It holds a
 * shared flock on ARC_ATTRIBUTION_MEMBERS for its whole life (the
 * descriptor is inherited by every fork and exec, so a descendant holds it
 * from birth), and it exits as soon as the test no longer holds its
 * exclusive flock on ARC_ATTRIBUTION_LIFELINE: at once when the test has
 * already let go, and from every loop otherwise. The test owns the set
 * before it starts any process, ends it on success or failure by releasing
 * the lifeline, and knows the set is empty when it can lock the members
 * file exclusively.
 */
#include <fcntl.h>
#include <poll.h>
#include <sys/mman.h>
#include <signal.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <errno.h>
#include <sys/file.h>
#include <sys/stat.h>
#include <sys/socket.h>
#include <sys/un.h>
#include <sys/wait.h>
#include <unistd.h>

static char dir[900];
static const char *lifeline;

/* Whether the test still holds the lifeline; any failure to tell is "no". */
static int alive(void) {
    int fd = open(lifeline, O_RDONLY | O_CLOEXEC);
    if (fd < 0) {
        return 0;
    }
    int held = flock(fd, LOCK_SH | LOCK_NB) != 0 && errno == EWOULDBLOCK;
    close(fd);
    return held;
}

static void join(void) {
    lifeline = getenv("ARC_ATTRIBUTION_LIFELINE");
    const char *members = getenv("ARC_ATTRIBUTION_MEMBERS");
    if (!lifeline || !members) {
        _exit(65);
    }
    /* Not close-on-exec: every descendant holds it from birth. */
    int fd = open(members, O_RDONLY);
    if (fd < 0 || flock(fd, LOCK_SH) != 0) {
        _exit(66);
    }
    if (!alive()) {
        _exit(0);
    }
}

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

static void set_dir(const char *value) {
    if (!value || strlen(value) >= sizeof dir) {
        _exit(64);
    }
    snprintf(dir, sizeof dir, "%s", value);
}

static int observe_self(const char *role, FILE **out) {
    const char *socket_path = getenv("ARC_ATTRIBUTION_SELF_OBSERVE");
    if (!socket_path || strlen(socket_path) >= sizeof(((struct sockaddr_un *)0)->sun_path)) {
        _exit(75);
    }
    int fd = socket(AF_UNIX, SOCK_STREAM, 0);
    struct sockaddr_un address = {0};
    address.sun_family = AF_UNIX;
    snprintf(address.sun_path, sizeof address.sun_path, "%s", socket_path);
    if (fd < 0 || connect(fd, (struct sockaddr *)&address, sizeof address) < 0) {
        _exit(76);
    }
    char name[128], output[1024];
    snprintf(name, sizeof name, "%s.observation", role);
    path(output, sizeof output, name);
    *out = fopen(output, "w");
    if (!*out) _exit(77);
    return fd;
}

/* Burn until <dir>/release, or until `counted_limit` (when non-zero)
 * observation lines that start with 'c' ("counted ...") were received. */
static unsigned long burn_until(const char *role, int counted_limit, const char *release) {
    FILE *observations = NULL;
    int fd = observe_self(role, &observations);
    volatile unsigned long x = 1;
    int counted = 0, line_start = 1;
    while (!exists(release) && (counted_limit == 0 || counted < counted_limit)) {
        if (!alive()) {
            _exit(0);
        }
        for (int i = 0; i < 2000000; i++) {
            x = x * 6364136223846793005UL + 1442695040888963407UL;
        }
        char buf[4096];
        ssize_t n = recv(fd, buf, sizeof buf, MSG_DONTWAIT);
        if (n > 0 && (fwrite(buf, 1, (size_t)n, observations) != (size_t)n || fflush(observations))) {
            _exit(78);
        }
        for (ssize_t i = 0; i < n; i++) {
            counted += line_start && buf[i] == 'c';
            line_start = buf[i] == '\n';
        }
    }
    fclose(observations);
    close(fd);
    return x;
}

/* Record lines without using CPU until a line arrives after <dir>/cycled. */
static int sleep_observe(const char *role) {
    char name[128];
    snprintf(name, sizeof name, "%s.pid", role);
    publish(name, (long)getpid());
    FILE *observations = NULL;
    int fd = observe_self(role, &observations);
    while (!exists("release")) {
        if (!alive()) {
            _exit(0);
        }
        struct pollfd wait = {fd, POLLIN, 0};
        if (poll(&wait, 1, 100) <= 0) {
            continue;
        }
        char buf[4096];
        ssize_t n = recv(fd, buf, sizeof buf, 0);
        if (n <= 0) {
            break;
        }
        if (fwrite(buf, 1, (size_t)n, observations) != (size_t)n || fflush(observations)) {
            _exit(78);
        }
        if (buf[n - 1] == '\n' && exists("cycled")) {
            break;
        }
    }
    fclose(observations);
    close(fd);
    return 0;
}

#define WORKERS 16
/* PIDs left below the target while respawn-1 waits for its next scan. */
#define MARGIN 1500

struct shared {
    volatile int stop, hit;
};

/* Allocate PIDs in WORKERS processes until `done`: phase 0 stops just below
 * `target` after the allocator wrapped; phase 1 gives `target` to a child
 * that execs `args`, or stops once a PID above it shows it was taken. */
/* vfork: the PID cycle needs ~100,000 allocations within one test, and its
 * children only call getpid and exec or _exit. */
#pragma clang diagnostic push
#pragma clang diagnostic ignored "-Wdeprecated-declarations"
static void allocate(struct shared *shared, pid_t target, int phase, char *const args[],
                     pid_t workers[WORKERS]) {
    pid_t low = target > MARGIN ? target - MARGIN : 1;
    shared->stop = 0;
    for (int w = 0; w < WORKERS; w++) {
        pid_t worker = fork();
        if (worker < 0) {
            _exit(70);
        }
        if (worker > 0) {
            workers[w] = worker;
            continue;
        }
        pid_t previous = 0;
        int wrapped = 0;
        unsigned long forks = 0;
        /* Phase 0 only cycles the allocator: let the kernel reap its exited
         * children instead of a waitpid round trip per PID. */
        if (phase == 0) {
            signal(SIGCHLD, SIG_IGN);
        }
        while (!shared->stop) {
            if ((++forks & 255) == 0 && (!alive() || exists("release"))) {
                shared->stop = 1;
                _exit(0);
            }
            pid_t p = vfork();
            if (p == 0) {
                if (phase == 1 && getpid() == target) {
                    execv(args[0], args);
                    _exit(74);
                }
                _exit(0);
            }
            if (p < 0) {
                continue;
            }
            if (phase == 1 && p == target) {
                shared->hit = 1;
                shared->stop = 1;
                int status = 0;
                _exit(waitpid(p, &status, 0) == p && WIFEXITED(status) ? WEXITSTATUS(status) : 71);
            }
            if (phase == 1) {
                waitpid(p, NULL, 0);
            }
            wrapped |= p < previous;
            previous = p;
            if ((phase == 0 && wrapped && p >= low) || (phase == 1 && p > target)) {
                shared->stop = 1;
            }
        }
        _exit(0);
    }
}
#pragma clang diagnostic pop

static int respawn(const char *burner) {
    publish("respawner.pid", (long)getpid());
    struct shared *shared =
        mmap(NULL, sizeof *shared, PROT_READ | PROT_WRITE, MAP_SHARED | MAP_ANON, -1, 0);
    if (shared == MAP_FAILED) {
        return 70;
    }
    shared->hit = 0;
    pid_t first = fork();
    if (first < 0) {
        return 70;
    }
    if (first == 0) {
        char *args[] = {(char *)burner, "sleep-observe", "respawn-1", dir, NULL};
        execv(burner, args);
        _exit(74);
    }
    pid_t workers[WORKERS];
    int status = 0;
    allocate(shared, first, 0, NULL, workers);
    for (int w = 0; w < WORKERS; w++) {
        waitpid(workers[w], NULL, 0);
    }
    if (exists("release")) {
        int reaped = waitpid(first, &status, 0) == first && WIFEXITED(status) &&
                     WEXITSTATUS(status) == 0;
        publish("reuse", 0);
        return reaped ? 0 : 71;
    }
    publish("cycled", 1);
    if (waitpid(first, &status, 0) != first || !WIFEXITED(status) || WEXITSTATUS(status) != 0) {
        return 71;
    }
    char *second[] = {(char *)burner, "burn", "respawn-2", dir, NULL};
    allocate(shared, first, 1, second, workers);
    /* Publish before the workers are waited for: the second life burns until release. */
    while (!shared->stop) {
        if (!alive()) {
            _exit(0);
        }
        usleep(1000);
    }
    usleep(100000);
    publish("reuse", shared->hit);
    int failed = 0;
    for (int w = 0; w < WORKERS; w++) {
        failed |= waitpid(workers[w], &status, 0) != workers[w] || !WIFEXITED(status) ||
                  WEXITSTATUS(status) != 0;
    }
    return failed ? 71 : 0;
}

static int burn(const char *role, int counted_limit) {
    char name[128];
    snprintf(name, sizeof name, "%s.pid", role);
    publish(name, (long)getpid());
    (void)burn_until(role, counted_limit, "release");
    return 0;
}

static int run_and_wait(char *const args[]) {
    pid_t child = fork();
    if (child < 0) {
        return 70;
    }
    if (child == 0) {
        execv(args[0], args);
        _exit(74);
    }
    int status = 0;
    return waitpid(child, &status, 0) == child && WIFEXITED(status) ? WEXITSTATUS(status) : 71;
}

int main(int argc, char **argv) {
    join();
    if (argc == 2 && strcmp(argv[1], "build") == 0) {
        set_dir(getenv("ARC_ATTRIBUTION_DIR"));
        int detached = getenv("ARC_ATTRIBUTION_DETACH_CHILD") != NULL;
        publish("root.pid", (long)getpid());
        pid_t child = fork();
        if (child < 0) {
            return 70;
        }
        if (child == 0) {
            if (detached) {
                publish("child.pid", (long)getpid());
                (void)burn_until("child", 0, "child-release");
                _exit(0);
            }
            _exit(burn("child", 0));
        }
        (void)burn_until("root", 0, "release");
        if (detached) return 0;
        int status = 0;
        if (waitpid(child, &status, 0) != child || !WIFEXITED(status) || WEXITSTATUS(status) != 0) {
            return 71;
        }
        return 0;
    }
    if (argc >= 5 && strcmp(argv[1], "detach") == 0) {
        pid_t middle = fork();
        if (middle < 0) {
            return 70;
        }
        if (middle == 0) {
            if (setsid() < 0) {
                _exit(72);
            }
            pid_t detached = fork();
            if (detached == 0) {
                int null = open("/dev/null", O_RDWR);
                if (null < 0 || dup2(null, 0) < 0 || dup2(null, 1) < 0 || dup2(null, 2) < 0) {
                    _exit(73);
                }
                close(null);
                execv(argv[2], argv + 2);
                _exit(74);
            }
            _exit(detached < 0 ? 70 : 0);
        }
        int status = 0;
        if (waitpid(middle, &status, 0) != middle || !WIFEXITED(status)) {
            return 71;
        }
        return WEXITSTATUS(status);
    }
    if (argc == 4 && strcmp(argv[1], "agent") == 0) {
        set_dir(argv[2]);
        publish("agent.pid", (long)getpid());
        char *args[] = {argv[3], "burn", "agent-burner", argv[2], NULL};
        return run_and_wait(args);
    }
    if (argc == 4 && strcmp(argv[1], "respawn") == 0) {
        set_dir(argv[2]);
        return respawn(argv[3]);
    }
    if (argc == 4 && strcmp(argv[1], "sleep-observe") == 0) {
        set_dir(argv[3]);
        return sleep_observe(argv[2]);
    }
    if (argc == 4 && strcmp(argv[1], "burn") == 0) {
        set_dir(argv[3]);
        return burn(argv[2], 0);
    }
    return 64;
}
