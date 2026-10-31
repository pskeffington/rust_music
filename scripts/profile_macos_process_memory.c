/*
 * Read current resident size and physical footprint for one local macOS PID.
 * Build with:
 *   clang -Wall -Wextra -O2 scripts/profile_macos_process_memory.c -o /tmp/rm-memory
 * Run with:
 *   /tmp/rm-memory <pid>
 */
#include <errno.h>
#include <inttypes.h>
#include <libproc.h>
#include <limits.h>
#include <stdio.h>
#include <stdlib.h>
#include <sys/resource.h>

int main(int argc, char **argv) {
    char *end = NULL;
    long parsed_pid;
    struct rusage_info_v4 usage = {0};

    if (argc != 2) {
        fprintf(stderr, "usage: %s PID\n", argv[0]);
        return 2;
    }

    errno = 0;
    parsed_pid = strtol(argv[1], &end, 10);
    if (errno != 0 || end == argv[1] || *end != '\0' || parsed_pid <= 0 ||
        parsed_pid > INT_MAX) {
        fprintf(stderr, "invalid process ID: %s\n", argv[1]);
        return 2;
    }

    if (proc_pid_rusage((int)parsed_pid, RUSAGE_INFO_V4,
                        (rusage_info_t *)&usage) != 0) {
        perror("proc_pid_rusage");
        return 1;
    }

    printf("pid=%ld resident_bytes=%" PRIu64 " physical_footprint_bytes=%" PRIu64
           "\n",
           parsed_pid, usage.ri_resident_size, usage.ri_phys_footprint);
    return 0;
}
