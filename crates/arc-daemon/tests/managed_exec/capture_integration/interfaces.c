/* Test-only process interposition: classification sees no host interfaces. */
#include <ifaddrs.h>
#include <fcntl.h>
#include <netinet/in.h>
#include <stddef.h>
#include <stdlib.h>
#include <string.h>
#include <sys/socket.h>
#include <sys/uio.h>
#include <sys/un.h>
#include <unistd.h>

static struct ifaddrs fixture_interface;
static struct sockaddr_in fixture_address;

static int fixture_getifaddrs(struct ifaddrs **result) {
    const char *marker = getenv("ARC_TEST_INTERFACE_MARKER");
    if (marker != NULL) {
        int fd = open(marker, O_WRONLY | O_CREAT | O_APPEND, 0600);
        if (fd >= 0) {
            (void)write(fd, "x", 1);
            (void)close(fd);
        }
    }
    memset(&fixture_interface, 0, sizeof(fixture_interface));
    memset(&fixture_address, 0, sizeof(fixture_address));
#ifdef __APPLE__
    fixture_address.sin_len = sizeof(fixture_address);
#endif
    fixture_address.sin_family = AF_INET;
    fixture_address.sin_addr.s_addr = htonl(0xc0000205); /* 192.0.2.5 */
    fixture_interface.ifa_addr = (struct sockaddr *)&fixture_address;
    *result = &fixture_interface;
    return 0;
}

static void fixture_freeifaddrs(struct ifaddrs *value) { (void)value; }

/* Optional, nonpersistent observation of the debug CLI capture seam. */
void arc_test_capture_observe(const unsigned char *route, size_t route_len,
                              const unsigned char *json, size_t json_len) {
    const char *path = getenv("ARC_TEST_CAPTURE_SOCKET");
    if (path == NULL) return;
    struct sockaddr_un target;
    memset(&target, 0, sizeof(target));
    size_t length = strlen(path);
    if (length == 0 || length >= sizeof(target.sun_path)) return;
    target.sun_family = AF_UNIX;
    memcpy(target.sun_path, path, length + 1);
    socklen_t target_len = (socklen_t)(offsetof(struct sockaddr_un, sun_path) + length + 1);
#ifdef __APPLE__
    target.sun_len = (unsigned char)target_len;
#endif
    int fd = socket(AF_UNIX, SOCK_DGRAM, 0);
    if (fd < 0) return;
    struct iovec parts[] = {
        {(void *)route, route_len}, {(void *)"\t", 1}, {(void *)json, json_len}
    };
    struct msghdr message;
    memset(&message, 0, sizeof(message));
    message.msg_name = &target;
    message.msg_namelen = target_len;
    message.msg_iov = parts;
    message.msg_iovlen = sizeof(parts) / sizeof(parts[0]);
    (void)sendmsg(fd, &message, 0);
    (void)close(fd);
}

#ifdef __APPLE__
__attribute__((used, section("__DATA,__interpose")))
static const struct { const void *replacement; const void *original; } hooks[] = {
    {(const void *)fixture_getifaddrs, (const void *)getifaddrs},
    {(const void *)fixture_freeifaddrs, (const void *)freeifaddrs},
};
#else
int getifaddrs(struct ifaddrs **result) { return fixture_getifaddrs(result); }
void freeifaddrs(struct ifaddrs *value) { fixture_freeifaddrs(value); }
#endif
