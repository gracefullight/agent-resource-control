// Independent macOS observer: no ARC library, IPC or daemon self-report.
// Native window metadata only; no images, screen capture, input or GUI control.
#include <CoreGraphics/CoreGraphics.h>
#include <errno.h>
#include <inttypes.h>
#include <libproc.h>
#include <mach/mach.h>
#include <mach/mach_time.h>
#include <mach/processor_info.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/mount.h>
#include <sys/sysctl.h>
#include <sys/time.h>
#include <time.h>
#include <unistd.h>

// RUSAGE_INFO_V4 CPU counters are Mach absolute-time ticks, not nanoseconds.
// Convert each counter before addition to avoid overflowing a uint64_t sum.
static double cpu_seconds(uint64_t user, uint64_t system, uint32_t numer, uint32_t denom) {
    return ((double)user * numer / denom + (double)system * numer / denom) / 1e9;
}
static mach_timebase_info_data_t cpu_timebase;

static double monotonic_ms(void) {
    struct timespec t;
    if (clock_gettime(CLOCK_MONOTONIC, &t)) { perror("clock_gettime"); exit(1); }
    return (double)t.tv_sec * 1000.0 + (double)t.tv_nsec / 1000000.0;
}
static double wall_ms(void) {
    struct timeval t;
    if (gettimeofday(&t, NULL)) { perror("gettimeofday"); exit(1); }
    return (double)t.tv_sec * 1000.0 + (double)t.tv_usec / 1000.0;
}
static void text(const char *value) {
    putchar('"');
    for (const unsigned char *p = (const unsigned char *)value; *p; p++) {
        if (*p == '"' || *p == '\\') { putchar('\\'); putchar(*p); }
        else if (*p < 32) printf("\\u%04x", *p);
        else putchar(*p);
    }
    putchar('"');
}
static int sysctl_read(const char *name, void *out, size_t bytes) {
    size_t size = bytes;
    return sysctlbyname(name, out, &size, NULL, 0) == 0 && size == bytes;
}
static void process(pid_t pid) {
    struct proc_bsdinfo bsd = {0};
    struct proc_taskinfo task = {0};
    struct rusage_info_v4 usage = {0};
    char path[PROC_PIDPATHINFO_MAXSIZE] = {0}, boot[37] = {0};
    size_t size = sizeof(boot);
    if (proc_pidinfo(pid, PROC_PIDTBSDINFO, 0, &bsd, sizeof(bsd)) != sizeof(bsd) ||
        proc_pidinfo(pid, PROC_PIDTASKINFO, 0, &task, sizeof(task)) != sizeof(task) ||
        proc_pid_rusage(pid, RUSAGE_INFO_V4, (rusage_info_t *)&usage) ||
        proc_pidpath(pid, path, sizeof(path)) <= 0 ||
        sysctlbyname("kern.bootsessionuuid", boot, &size, NULL, 0) || size != sizeof(boot)) {
        perror("owned process observation"); exit(1);
    }
    printf("{\"pid\":%d,\"startToken\":\"%" PRIu64 "\",\"bootId\":", pid,
        bsd.pbi_start_tvsec * UINT64_C(1000000) + bsd.pbi_start_tvusec);
    for (size_t i = 0; i < sizeof(boot); i++) if (boot[i] >= 'A' && boot[i] <= 'Z') boot[i] += 'a' - 'A';
    text(boot); printf(",\"executable\":"); text(path);
    printf(",\"cpuCoreSeconds\":%.9f,\"userCpuTicks\":%" PRIu64 ",\"systemCpuTicks\":%" PRIu64
        ",\"cpuTimebaseNumer\":%u,\"cpuTimebaseDenom\":%u,\"cpuCounterUnit\":\"mach_absolute_ticks\""
        ",\"cpuUserSeconds\":%.9f,\"cpuSystemSeconds\":%.9f,\"rssBytes\":%" PRIu64 ",\"rusageResidentBytes\":%" PRIu64
        ",\"source\":\"proc_pid_rusage(RUSAGE_INFO_V4) Mach ticks converted with mach_timebase_info + proc_pidinfo(PROC_PIDTASKINFO,PROC_PIDTBSDINFO)\"}",
        cpu_seconds(usage.ri_user_time, usage.ri_system_time, cpu_timebase.numer, cpu_timebase.denom),
        usage.ri_user_time, usage.ri_system_time, cpu_timebase.numer, cpu_timebase.denom,
        cpu_seconds(usage.ri_user_time, 0, cpu_timebase.numer, cpu_timebase.denom),
        cpu_seconds(0, usage.ri_system_time, cpu_timebase.numer, cpu_timebase.denom),
        task.pti_resident_size, usage.ri_resident_size);
}
static void windows(pid_t pid) {
    CFArrayRef list = CGWindowListCopyWindowInfo(kCGWindowListOptionAll | kCGWindowListExcludeDesktopElements, kCGNullWindowID);
    if (!list) { fprintf(stderr, "native window metadata unavailable\n"); exit(1); }
    unsigned owned = 0, onscreen = 0, normal_onscreen = 0;
    for (CFIndex i = 0; i < CFArrayGetCount(list); i++) {
        CFDictionaryRef row = CFArrayGetValueAtIndex(list, i);
        int owner = 0;
        CFNumberRef number = CFDictionaryGetValue(row, kCGWindowOwnerPID);
        if (!number || !CFNumberGetValue(number, kCFNumberIntType, &owner) || owner != pid) continue;
        owned++;
        int layer = 0;
        CFNumberRef layer_number = CFDictionaryGetValue(row, kCGWindowLayer);
        if (!layer_number || !CFNumberGetValue(layer_number, kCFNumberIntType, &layer)) {
            CFRelease(list); fprintf(stderr, "native window layer unavailable\n"); exit(1);
        }
        CFBooleanRef shown = CFDictionaryGetValue(row, kCGWindowIsOnscreen);
        if (shown && CFBooleanGetValue(shown)) { onscreen++; if (layer == 0) normal_onscreen++; }
    }
    CFRelease(list);
    printf("{\"pid\":%d,\"windowCount\":%u,\"onscreenCount\":%u,\"normalOnscreenCount\":%u,\"source\":\"CGWindowListCopyWindowInfo metadata; no image capture\"}", pid, owned, onscreen, normal_onscreen);
}
static void host(void) {
    mach_port_t port = mach_host_self();
    natural_t cpus = 0;
    processor_info_array_t info = NULL;
    mach_msg_type_number_t count = 0;
    kern_return_t result = host_processor_info(port, PROCESSOR_CPU_LOAD_INFO, &cpus, &info, &count);
    printf("\"host\":{\"cpu\":");
    if (result == KERN_SUCCESS && count == cpus * CPU_STATE_MAX) {
        uint64_t ticks[CPU_STATE_MAX] = {0};
        for (unsigned i = 0; i < count; i++) ticks[i % CPU_STATE_MAX] += (uint32_t)info[i];
        printf("{\"user\":%" PRIu64 ",\"system\":%" PRIu64 ",\"nice\":%" PRIu64 ",\"idle\":%" PRIu64
            ",\"logicalCpuCount\":%u,\"source\":\"host_processor_info per-CPU tick sum\"}",
            ticks[CPU_STATE_USER], ticks[CPU_STATE_SYSTEM], ticks[CPU_STATE_NICE], ticks[CPU_STATE_IDLE], cpus);
    } else printf("null");
    if (info) vm_deallocate(mach_task_self(), (vm_address_t)info, count * sizeof(integer_t));
    vm_statistics64_data_t vm = {0};
    count = HOST_VM_INFO64_COUNT;
    vm_size_t page = 0;
    printf(",\"memory\":");
    if (host_statistics64(port, HOST_VM_INFO64, (host_info64_t)&vm, &count) == KERN_SUCCESS && host_page_size(port, &page) == KERN_SUCCESS) {
        printf("{\"pageSize\":%u,\"freePages\":%u,\"inactivePages\":%u,\"availableBytes\":%" PRIu64
            ",\"source\":\"HOST_VM_INFO64 free+inactive; host_page_size\"}",
            (unsigned)page, vm.free_count, vm.inactive_count, ((uint64_t)vm.free_count + vm.inactive_count) * page);
    } else printf("null");
    mach_port_deallocate(mach_task_self(), port);
    int pressure = 0;
    printf(",\"pressureRaw\":");
    if (sysctl_read("kern.memorystatus_vm_pressure_level", &pressure, sizeof(pressure))) printf("%d", pressure); else printf("null");
    struct xsw_usage swap = {0};
    printf(",\"swapUsedBytes\":");
    if (sysctl_read("vm.swapusage", &swap, sizeof(swap))) printf("%" PRIu64, swap.xsu_used); else printf("null");
    struct statfs disk = {0};
    printf(",\"filesystem\":");
    if (statfs("/", &disk) == 0) {
        printf("{\"path\":\"/\",\"mountPoint\":"); text(disk.f_mntonname);
        printf(",\"filesystemType\":"); text(disk.f_fstypename);
        printf(",\"capacityBytes\":%" PRIu64 ",\"freeBytes\":%" PRIu64 ",\"availableBytes\":%" PRIu64
            ",\"source\":\"independent statfs(/) block counts\"}",
            disk.f_blocks * disk.f_bsize, disk.f_bfree * disk.f_bsize, disk.f_bavail * disk.f_bsize);
    } else printf("null");
    printf("}");
}
int main(int argc, char **argv) {
    if (mach_timebase_info(&cpu_timebase) != KERN_SUCCESS || !cpu_timebase.numer || !cpu_timebase.denom) {
        fprintf(stderr, "native CPU timebase unavailable\n"); return 1;
    }
    if (argc == 2 && strcmp(argv[1], "host") == 0) {
        printf("{\"atMs\":%.3f,\"monotonicMs\":%.3f,", wall_ms(), monotonic_ms());
        host(); puts("}"); return 0;
    }
    if (argc < 5 || argc > 6 || strcmp(argv[1], "observe") != 0) {
        fprintf(stderr, "usage: native-observer host | observe PID COUNT INTERVAL_MS [DESKTOP_PID]\n"); return 2;
    }
    char *end = NULL;
    long pid = strtol(argv[2], &end, 10); if (!end || *end || pid <= 0 || pid > INT32_MAX) return 2;
    long count = strtol(argv[3], &end, 10); if (!end || *end || count < 1 || count > 1000) return 2;
    long interval = strtol(argv[4], &end, 10); if (!end || *end || interval < 10 || interval > 5000) return 2;
    long desktop = 0;
    if (argc == 6) { desktop = strtol(argv[5], &end, 10); if (!end || *end || desktop <= 0 || desktop > INT32_MAX) return 2; }
    for (long i = 0; i < count; i++) {
        printf("{\"atMs\":%.3f,\"monotonicMs\":%.3f,\"process\":", wall_ms(), monotonic_ms()); process((pid_t)pid);
        if (desktop) { printf(",\"desktopProcess\":"); process((pid_t)desktop); printf(",\"windows\":"); windows((pid_t)desktop); }
        puts("}"); fflush(stdout);
        if (i + 1 < count) {
            struct timespec t = { .tv_sec = interval / 1000, .tv_nsec = (interval % 1000) * 1000000 };
            while (nanosleep(&t, &t) && errno == EINTR) {}
        }
    }
    return 0;
}
