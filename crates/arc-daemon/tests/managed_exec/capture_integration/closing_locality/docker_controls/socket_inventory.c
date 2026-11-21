/* Offline interposition for the Docker listener inventory; no socket is contacted. */
#include <libproc.h>
#include <stdlib.h>
#include <string.h>
#include <sys/proc_info.h>
#include <sys/socket.h>
#include <unistd.h>

#define FIRST_PID 777771
#define SECOND_PID 777772
static const char *fixture_path(void) { return getenv("ARC_TEST_DOCKER_SOCKET"); }
static int fixture_pid(int pid) { return pid == 1 || pid == FIRST_PID || pid == SECOND_PID; }

static int fake_listpids(uint32_t type, uint32_t info, void *buffer, int size) {
    const char *mode = getenv("ARC_TEST_DOCKER_LISTENER");
    if (!fixture_path()) {
        return proc_listpids(type, info, buffer, size);
    }
    if (mode && !strcmp(mode, "unreadable")) return -1;
    if (size < (int)(2 * sizeof(int))) return -1;
    ((int *)buffer)[0] = FIRST_PID;
    if (mode && (!strcmp(mode, "multiple") || !strcmp(mode, "pid1-shared") ||
                 !strcmp(mode, "pid1-unreadable"))) {
        ((int *)buffer)[1] = !strcmp(mode, "multiple") ? SECOND_PID : 1;
        return (int)(2 * sizeof(int));
    }
    return (int)sizeof(int);
}

static int fake_pidinfo(int pid, int flavor, uint64_t arg, void *buffer, int size) {
    if (!fixture_path() || !fixture_pid(pid)) {
        return proc_pidinfo(pid, flavor, arg, buffer, size);
    }
    const char *mode = getenv("ARC_TEST_DOCKER_LISTENER");
    if (pid == 1 && flavor == PROC_PIDLISTFDS && mode &&
        !strcmp(mode, "pid1-unreadable")) return 0;
    if (flavor == PROC_PIDTBSDINFO && size >= (int)sizeof(struct proc_bsdinfo)) {
        struct proc_bsdinfo *info = buffer;
        memset(info, 0, sizeof(*info));
        info->pbi_pid = pid;
        /* An additional listener can belong to a different UID. */
        info->pbi_uid = pid == 1 ? 0 : (pid == SECOND_PID ? getuid() + 1 : getuid());
        info->pbi_start_tvsec = 100;
        return (int)sizeof(*info);
    }
    if (flavor == PROC_PIDLISTFDS && size >= (int)sizeof(struct proc_fdinfo)) {
        struct proc_fdinfo *fd = buffer;
        fd->proc_fd = 42;
        fd->proc_fdtype = PROX_FDTYPE_SOCKET;
        return (int)sizeof(*fd);
    }
    return 0;
}

static int fake_pidfdinfo(int pid, int fd, int flavor, void *buffer, int size) {
    if (!fixture_path() || !fixture_pid(pid)) {
        return proc_pidfdinfo(pid, fd, flavor, buffer, size);
    }
    if (fd != 42 || flavor != PROC_PIDFDSOCKETINFO || size < (int)sizeof(struct socket_fdinfo)) return 0;
    struct socket_fdinfo *info = buffer;
    memset(info, 0, sizeof(*info));
    info->psi.soi_so = (uint64_t)pid;
    info->psi.soi_family = AF_UNIX;
    info->psi.soi_type = SOCK_STREAM;
    info->psi.soi_kind = SOCKINFO_UN;
    const char *mode = getenv("ARC_TEST_DOCKER_LISTENER");
    if (!mode || strcmp(mode, "not-listening")) info->psi.soi_options = SO_ACCEPTCONN;
    size_t length = strlen(fixture_path());
    if (length >= sizeof(info->psi.soi_proto.pri_un.unsi_addr.ua_sun.sun_path)) return 0;
    memcpy(info->psi.soi_proto.pri_un.unsi_addr.ua_sun.sun_path, fixture_path(), length + 1);
    return (int)sizeof(*info);
}

static int fake_pidpath(int pid, void *buffer, uint32_t size) {
    if (!fixture_path() || !fixture_pid(pid)) {
        return proc_pidpath(pid, buffer, size);
    }
    const char *mode = getenv("ARC_TEST_DOCKER_LISTENER");
    const char *path = pid == 1 ? "/sbin/launchd" :
        (mode && !strcmp(mode, "ssh") ? "/usr/bin/ssh" : "/fixture/dockerd");
    size_t length = strlen(path);
    if (size <= length) return 0;
    memcpy(buffer, path, length + 1);
    return (int)length;
}

__attribute__((used, section("__DATA,__interpose")))
static const struct { const void *replacement; const void *original; } hooks[] = {
    {(const void *)fake_listpids, (const void *)proc_listpids},
    {(const void *)fake_pidinfo, (const void *)proc_pidinfo},
    {(const void *)fake_pidfdinfo, (const void *)proc_pidfdinfo},
    {(const void *)fake_pidpath, (const void *)proc_pidpath},
};
