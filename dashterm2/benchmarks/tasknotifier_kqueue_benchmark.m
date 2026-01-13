// tasknotifier_kqueue_benchmark.m
// Benchmark comparing select() vs kqueue() for the TaskNotifier I/O polling pattern.
//
// Build (macOS):
//   clang -framework Foundation -fobjc-arc -O3 \
//     benchmarks/tasknotifier_kqueue_benchmark.m -o benchmarks/tasknotifier_kqueue_benchmark
//
// Run:
//   ./benchmarks/tasknotifier_kqueue_benchmark
//
// This benchmark simulates the I/O polling pattern used by DashTerm2's TaskNotifier,
// comparing:
// 1. select() - O(n) with FD_SETSIZE limitation
// 2. kqueue() - O(1) event retrieval, no FD limit

#import <Foundation/Foundation.h>
#import <sys/event.h>
#import <sys/select.h>
#import <sys/time.h>
#import <unistd.h>
#import <fcntl.h>
#import <mach/mach_time.h>

// Number of simulated terminal sessions (pipe pairs)
static const int kTaskCounts[] = {1, 4, 16, 64, 256};
static const int kNumTaskConfigs = sizeof(kTaskCounts) / sizeof(kTaskCounts[0]);

// Number of polling iterations per benchmark
static const int kIterations = 10000;

// Number of warmup iterations
static const int kWarmupIterations = 1000;

// Mach time to nanoseconds conversion
static mach_timebase_info_data_t sTimebaseInfo;

static uint64_t machTimeToNanos(uint64_t machTime) {
    return machTime * sTimebaseInfo.numer / sTimebaseInfo.denom;
}

// Task simulation - represents a PTY file descriptor pair
typedef struct {
    int readFd;  // Master side read
    int writeFd; // Master side write (for unblock signal simulation)
} SimulatedTask;

// Create simulated tasks (pipe pairs)
static SimulatedTask *createTasks(int count) {
    SimulatedTask *tasks = malloc(count * sizeof(SimulatedTask));
    for (int i = 0; i < count; i++) {
        int pipeFds[2];
        if (pipe(pipeFds) != 0) {
            perror("pipe");
            exit(1);
        }
        // Set non-blocking
        fcntl(pipeFds[0], F_SETFL, O_NONBLOCK);
        fcntl(pipeFds[1], F_SETFL, O_NONBLOCK);
        tasks[i].readFd = pipeFds[0];
        tasks[i].writeFd = pipeFds[1];
    }
    return tasks;
}

// Destroy simulated tasks
static void destroyTasks(SimulatedTask *tasks, int count) {
    for (int i = 0; i < count; i++) {
        close(tasks[i].readFd);
        close(tasks[i].writeFd);
    }
    free(tasks);
}

// Trigger some events on random tasks (simulate I/O activity)
static void triggerEvents(SimulatedTask *tasks, int count, int numEvents) {
    for (int i = 0; i < numEvents && i < count; i++) {
        int idx = i % count;
        char c = 'x';
        write(tasks[idx].writeFd, &c, 1);
    }
}

// Drain events from tasks
static void drainEvents(SimulatedTask *tasks, int count) {
    char buf[64];
    for (int i = 0; i < count; i++) {
        while (read(tasks[i].readFd, buf, sizeof(buf)) > 0) {
        }
    }
}

#pragma mark - select() implementation

// Benchmark select()-based polling
static double benchmarkSelect(SimulatedTask *tasks, int count, int iterations) {
    uint64_t totalTime = 0;

    for (int iter = 0; iter < iterations; iter++) {
        // Trigger a few events
        triggerEvents(tasks, count, (iter % 3) + 1);

        fd_set rfds, wfds, efds;
        int highfd = 0;

        uint64_t start = mach_absolute_time();

        // Setup fd_sets (O(n) operation)
        FD_ZERO(&rfds);
        FD_ZERO(&wfds);
        FD_ZERO(&efds);

        for (int i = 0; i < count; i++) {
            int fd = tasks[i].readFd;
            if (fd >= FD_SETSIZE) {
                // select() cannot handle fd >= FD_SETSIZE
                continue;
            }
            FD_SET(fd, &rfds);
            FD_SET(fd, &efds);
            if (fd > highfd) {
                highfd = fd;
            }
        }

        // Poll with zero timeout (non-blocking)
        struct timeval timeout = {0, 0};
        int result = select(highfd + 1, &rfds, &wfds, &efds, &timeout);

        // Process events (O(n) scan)
        if (result > 0) {
            for (int i = 0; i < count; i++) {
                int fd = tasks[i].readFd;
                if (fd < FD_SETSIZE && FD_ISSET(fd, &rfds)) {
                    // Event on this fd - would call processRead
                }
            }
        }

        uint64_t end = mach_absolute_time();
        totalTime += (end - start);

        // Drain to reset for next iteration
        drainEvents(tasks, count);
    }

    return (double)machTimeToNanos(totalTime) / 1e6; // Return milliseconds
}

#pragma mark - kqueue() implementation

// Benchmark kqueue()-based polling
static double benchmarkKqueue(SimulatedTask *tasks, int count, int iterations) {
    int kq = kqueue();
    if (kq < 0) {
        perror("kqueue");
        return -1;
    }

    // Register all fds with kqueue (done once, not per-iteration)
    struct kevent *changes = malloc(count * sizeof(struct kevent));
    for (int i = 0; i < count; i++) {
        EV_SET(&changes[i], tasks[i].readFd, EVFILT_READ, EV_ADD | EV_CLEAR, 0, 0, NULL);
    }

    if (kevent(kq, changes, count, NULL, 0, NULL) < 0) {
        perror("kevent register");
        free(changes);
        close(kq);
        return -1;
    }

    struct kevent *events = malloc(count * sizeof(struct kevent));
    uint64_t totalTime = 0;

    for (int iter = 0; iter < iterations; iter++) {
        // Trigger a few events
        triggerEvents(tasks, count, (iter % 3) + 1);

        uint64_t start = mach_absolute_time();

        // Poll with zero timeout (non-blocking)
        struct timespec timeout = {0, 0};
        int numEvents = kevent(kq, NULL, 0, events, count, &timeout);

        // Process events (O(numEvents), not O(count))
        if (numEvents > 0) {
            for (int i = 0; i < numEvents; i++) {
                // Event on events[i].ident - would call processRead
                (void)events[i].ident;
            }
        }

        uint64_t end = mach_absolute_time();
        totalTime += (end - start);

        // Drain to reset for next iteration
        drainEvents(tasks, count);
    }

    free(changes);
    free(events);
    close(kq);

    return (double)machTimeToNanos(totalTime) / 1e6; // Return milliseconds
}

#pragma mark - Main

int main(int argc, const char *argv[]) {
    @autoreleasepool {
        mach_timebase_info(&sTimebaseInfo);

        printf("TaskNotifier I/O Polling Benchmark: select() vs kqueue()\n");
        printf("=========================================================\n");
        printf("Iterations per config: %d\n", kIterations);
        printf("Warmup iterations: %d\n\n", kWarmupIterations);

        printf("%-10s %15s %15s %10s\n", "Tasks", "select() (ms)", "kqueue() (ms)", "Speedup");
        printf("---------- --------------- --------------- ----------\n");

        double totalSelectTime = 0;
        double totalKqueueTime = 0;

        for (int c = 0; c < kNumTaskConfigs; c++) {
            int taskCount = kTaskCounts[c];

            // Skip if too many fds for select
            if (taskCount * 2 >= FD_SETSIZE) {
                printf("%-10d %15s %15s %10s\n", taskCount, "N/A (FD_SETSIZE)", "-", "-");
                continue;
            }

            SimulatedTask *tasks = createTasks(taskCount);

            // Warmup
            benchmarkSelect(tasks, taskCount, kWarmupIterations);
            benchmarkKqueue(tasks, taskCount, kWarmupIterations);
            drainEvents(tasks, taskCount);

            // Benchmark
            double selectTime = benchmarkSelect(tasks, taskCount, kIterations);
            drainEvents(tasks, taskCount);
            double kqueueTime = benchmarkKqueue(tasks, taskCount, kIterations);

            double speedup = selectTime / kqueueTime;

            printf("%-10d %15.3f %15.3f %9.2fx\n", taskCount, selectTime, kqueueTime, speedup);

            totalSelectTime += selectTime;
            totalKqueueTime += kqueueTime;

            destroyTasks(tasks, taskCount);
        }

        printf("---------- --------------- --------------- ----------\n");

        if (totalKqueueTime > 0) {
            double overallSpeedup = totalSelectTime / totalKqueueTime;
            printf("%-10s %15.3f %15.3f %9.2fx\n", "Total", totalSelectTime, totalKqueueTime, overallSpeedup);
        }

        printf("\n");
        printf("Analysis:\n");
        printf("---------\n");
        printf("- select() complexity: O(n) setup + O(highfd) kernel scan + O(n) result scan\n");
        printf("- kqueue() complexity: O(1) poll + O(numEvents) result processing\n");
        printf("- select() limitation: Cannot handle fd >= %d\n", FD_SETSIZE);
        printf("- kqueue() limitation: None (limited by system resources)\n");
        printf("\n");
        printf("For typical terminal usage with 1-16 sessions, the difference is small.\n");
        printf("For heavy usage with many sessions, kqueue provides significant benefits.\n");
        printf("Additionally, kqueue removes the FD_SETSIZE limitation entirely.\n");
    }
    return 0;
}
