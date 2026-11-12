// Deterministic regression for reference-tool CPU units; no ARC implementation.
#define main native_observer_cli_main
#include "native-observer.c"
#undef main
#include <assert.h>
#include <math.h>

static void expect_seconds(uint64_t user, uint64_t system, uint32_t numer,
                           uint32_t denom, double expected) {
    double actual = cpu_seconds(user, system, numer, denom);
    assert(isfinite(actual));
    assert(fabs(actual - expected) <= fmax(1e-12, fabs(expected) * 1e-12));
}
int main(void) {
    expect_seconds(0, 0, 125, 3, 0);
    expect_seconds(12000000, 12000000, 125, 3, 1);
    expect_seconds(24000000, 0, 125, 3, 1);
    expect_seconds(0, 24000000, 125, 3, 1);
    expect_seconds(500000000, 500000000, 1, 1, 1);
    expect_seconds(1, 2, 125, 3, 125e-9);
    // Counter sum exceeds UINT64_MAX; conversion must not wrap before scaling.
    expect_seconds(UINT64_MAX, UINT64_MAX, 1, 1, 2.0 * (double)UINT64_MAX / 1e9);
    puts("{\"status\":\"passed\",\"cases\":7,\"covers\":[\"zero\",\"Apple-Silicon-125/3\",\"user-system-sum\",\"Intel-1/1\",\"submicrosecond\",\"sum-overflow\"]}");
    return 0;
}
