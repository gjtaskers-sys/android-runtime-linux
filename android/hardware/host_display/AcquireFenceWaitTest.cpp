#include "AcquireFenceWait.h"

#include <cassert>
#include <cerrno>
#include <cstdio>
#include <initializer_list>

namespace {
struct PollStep {
    int result;
    short revents;
    int error;
};

PollStep* steps = nullptr;
int step_count = 0;
int step_index = 0;

int scriptedPoll(pollfd* descriptors, nfds_t count, int timeout_ms) {
    assert(count == 1);
    assert(timeout_ms == -1);
    assert(descriptors[0].events == POLLIN);
    assert(step_index < step_count);
    const PollStep step = steps[step_index++];
    descriptors[0].revents = step.revents;
    if (step.result < 0) errno = step.error;
    return step.result;
}

bool run(std::initializer_list<PollStep> scenario) {
    // Each scenario is backed by a local fixed-size copy for the scripted poller.
    static PollStep local[8];
    assert(scenario.size() <= 8);
    step_count = 0;
    step_index = 0;
    for (const PollStep& step : scenario) local[step_count++] = step;
    steps = local;
    return android_runtime::waitForFenceReadableWith(11, scriptedPoll);
}

void test_readable_fence_succeeds() {
    assert(run({{1, POLLIN, 0}}));
}

void test_errors_hangups_and_empty_events_fail() {
    assert(!run({{1, static_cast<short>(POLLIN | POLLERR), 0}}));
    assert(!run({{1, POLLHUP, 0}}));
    assert(!run({{1, 0, 0}}));
    assert(!run({{0, 0, 0}}));
    assert(!run({{-1, 0, EIO}}));
}

void test_interrupted_poll_is_retried() {
    assert(run({{-1, 0, EINTR}, {1, POLLIN, 0}}));
    assert(step_index == 2);
}

}  // namespace

int main() {
    test_readable_fence_succeeds();
    test_errors_hangups_and_empty_events_fail();
    test_interrupted_poll_is_retried();
    std::puts("acquire-fence wait tests passed (readiness, error/hangup handling, EINTR retry)");
    return 0;
}
