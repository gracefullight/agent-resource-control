/* Native build/test payload. Readiness is a marker, release is an explicit file. */
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>
#include <time.h>
#include <unistd.h>

int main(int argc, char **argv) {
    const char *root = getenv("TBS_PROJECT"), *body = getenv("TBS_BODY");
    const char *release = getenv("TBS_RELEASE"), *value = getenv("TBS_VALUE");
    char cwd[4096], failure[4096];
    if (argc != 3 || !root || !body || !release || !value || !getcwd(cwd, sizeof(cwd))) return 125;
    if (strcmp(argv[1], "test") == 0) {
        struct stat out, err;
        /* CTest 4.4.5 supplies one shared pipe to both child descriptors.
           Establish that invariant instead of assuming cross-pipe ordering. */
        if (fstat(STDOUT_FILENO, &out) || fstat(STDERR_FILENO, &err) ||
            out.st_dev != err.st_dev || out.st_ino != err.st_ino) return 125;
    }
    FILE *file = fopen(body, "a");
    if (!file) return 125;
    fprintf(file, "%s %d\n", argv[1], getpid());
    if (fclose(file) != 0) return 125;
    printf("ordinary stdout before\nNATIVE_PHASE=%s\nNATIVE_VALUE=%s\nNATIVE_CWD=%s\nNATIVE_ARGUMENT=%s\nordinary stdout after\n",
           argv[1], value, cwd, argv[2]);
    /* CTest merges its test child's pipes. Complete stderr before flushing
       stdout so both direct and managed CTest runs have a stable ordering. */
    fprintf(stderr, "ordinary stderr before\nNATIVE_STDERR=%s\nordinary stderr after\n", argv[1]);
    fflush(stdout);
    struct timespec start, now;
    if (clock_gettime(CLOCK_MONOTONIC, &start) != 0) return 125;
    while (access(release, F_OK) != 0) {
        if (clock_gettime(CLOCK_MONOTONIC, &now) != 0 || now.tv_sec - start.tv_sec > 120) return 124;
        usleep(10000); /* Poll the explicit barrier; elapsed time never proves ordering. */
    }
    if (snprintf(failure, sizeof(failure), "%s/fail", root) >= (int)sizeof(failure)) return 125;
    return access(failure, F_OK) == 0 ? 17 : 0;
}
