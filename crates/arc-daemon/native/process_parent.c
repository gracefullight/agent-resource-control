/* The installed SDK owns the kinfo_proc layout, including on arm64. Query
 * only PID/parent PID. libproc's BSD-info calls exclude unreaped zombies on
 * Darwin; a positive-PID sysctl retains their parent until the sole CLI reaps.
 * No wait, signal, command, environment or process-group data is returned. */
#include <errno.h>
#include <string.h>
#include <sys/types.h>
#include <sys/sysctl.h>
#include <sys/proc.h>

int arc_unreaped_parent_pid(int pid, int *parent) {
    if (pid <= 0 || parent == NULL) return EINVAL;
    int mib[4] = {CTL_KERN, KERN_PROC, KERN_PROC_PID, pid};
    struct kinfo_proc info;
    memset(&info, 0, sizeof(info));
    size_t size = sizeof(info);
    if (sysctl(mib, 4, &info, &size, NULL, 0) != 0) return errno;
    if (size != sizeof(info) || info.kp_proc.p_pid != pid ||
        info.kp_proc.p_stat != SZOMB || info.kp_eproc.e_ppid <= 0) return ESRCH;
    *parent = info.kp_eproc.e_ppid;
    return 0;
}
