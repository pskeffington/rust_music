import Darwin
import Foundation

private let packageID = Data("org.rustboy.music".utf8)
private let deadline: TimeInterval = 5

private func require(_ condition: Bool, _ message: String) throws {
    guard condition else { throw NSError(domain: "ProcessTerminationTests", code: 1, userInfo: [NSLocalizedDescriptionKey: message]) }
}

private func appendBE<T: FixedWidthInteger>(_ value: T, to data: inout Data) {
    for shift in stride(from: T.bitWidth - 8, through: 0, by: -8) {
        data.append(UInt8(truncatingIfNeeded: value >> T(shift)))
    }
}

private func frame(_ requestID: UInt64, tag: UInt8, extra: Data = Data()) -> Data {
    var payload = Data([1, 1])
    appendBE(requestID, to: &payload)
    appendBE(UInt16(packageID.count), to: &payload)
    payload.append(packageID)
    payload.append(tag)
    payload.append(extra)
    var result = Data("RBPK".utf8)
    appendBE(UInt32(payload.count), to: &result)
    result.append(payload)
    return result
}

private func readExactly(_ handle: FileHandle, count: Int) throws -> Data {
    let end = Date().addingTimeInterval(deadline)
    var data = Data()
    while data.count < count {
        let remaining = end.timeIntervalSinceNow
        guard remaining > 0 else {
            throw NSError(domain: "ProcessTerminationTests", code: 2, userInfo: [NSLocalizedDescriptionKey: "Timed out reading package process reply."])
        }
        var descriptor = pollfd(
            fd: Int32(handle.fileDescriptor),
            events: Int16(POLLIN),
            revents: 0
        )
        let ready = Darwin.poll(&descriptor, 1, Int32(min(Double(Int32.max), remaining * 1000)))
        guard ready > 0 else {
            throw NSError(domain: "ProcessTerminationTests", code: 2, userInfo: [NSLocalizedDescriptionKey: "Timed out reading package process reply."])
        }
        var chunk = [UInt8](repeating: 0, count: count - data.count)
        let readCount = chunk.withUnsafeMutableBufferPointer { buffer in
            Darwin.read(handle.fileDescriptor, buffer.baseAddress, buffer.count)
        }
        guard readCount > 0 else {
            throw NSError(domain: "ProcessTerminationTests", code: 3, userInfo: [NSLocalizedDescriptionKey: "Package process closed a reply early."])
        }
        data.append(contentsOf: chunk.prefix(readCount))
    }
    return data
}

private func receiveBody(_ output: Pipe, requestID: UInt64) throws -> Data {
    let handle = output.fileHandleForReading
    let header = try readExactly(handle, count: 8)
    guard header.prefix(4) == Data("RBPK".utf8) else {
        throw NSError(domain: "ProcessTerminationTests", code: 4, userInfo: [NSLocalizedDescriptionKey: "Package child returned an invalid frame header."])
    }
    let length = header.suffix(4).reduce(UInt32(0)) { ($0 << 8) | UInt32($1) }
    guard (1...16_384).contains(length) else {
        throw NSError(domain: "ProcessTerminationTests", code: 5, userInfo: [NSLocalizedDescriptionKey: "Package child reply length exceeded its bound."])
    }
    let payload = try readExactly(handle, count: Int(length))
    guard payload.count >= 12,
          payload[0] == 1, payload[1] == 2,
          payload[2..<10].reduce(UInt64(0), { ($0 << 8) | UInt64($1) }) == requestID else {
        throw NSError(domain: "ProcessTerminationTests", code: 6, userInfo: [NSLocalizedDescriptionKey: "Package child reply envelope was invalid."])
    }
    let packageLength = payload[10..<12].reduce(UInt16(0), { ($0 << 8) | UInt16($1) })
    let packageEnd = 12 + Int(packageLength)
    guard packageLength == packageID.count,
          payload.count >= packageEnd,
          Data(payload[12..<packageEnd]) == packageID,
          payload.count > packageEnd else {
        throw NSError(domain: "ProcessTerminationTests", code: 6, userInfo: [NSLocalizedDescriptionKey: "Package child reply identity was invalid."])
    }
    return Data(payload[packageEnd...])
}

private func launch(_ executable: URL) throws -> (Process, Pipe, Pipe) {
    let process = Process()
    let input = Pipe()
    let output = Pipe()
    process.executableURL = executable
    process.arguments = ["serve", "--desktop"]
    process.standardInput = input
    process.standardOutput = output
    process.standardError = FileHandle.nullDevice
    try process.run()
    return (process, input, output)
}

private func cleanUpDirectChild(_ process: Process, input: Pipe, output: Pipe) {
    try? input.fileHandleForWriting.close()
    if process.isRunning {
        ProcessTermination.terminateAndWait(process, gracefulTimeout: 0.25, forcedTimeout: 1)
    }
    process.waitUntilExit()
    try? output.fileHandleForReading.close()
}

private final class TestProcessGroupGuard {
    private let group: pid_t
    private var cleaned = false

    init(group: pid_t) { self.group = group }

    func cleanUp() {
        guard !cleaned else { return }
        cleaned = true
        // parentMode retains a watchdog in this isolated group, so the group
        // identifier cannot be reused before this one-shot cleanup signal.
        if kill(-group, 0) == 0 { _ = kill(-group, SIGKILL) }
    }
}

private func testGracefulShutdown(_ executable: URL) throws {
    let (process, input, output) = try launch(executable)
    defer { cleanUpDirectChild(process, input: input, output: output) }
    input.fileHandleForWriting.write(frame(1, tag: 0, extra: Data([0, 1, 0, 1])))
    let hello = try receiveBody(output, requestID: 1)
    try require(hello == Data([0, 0, 1]), "Hello did not negotiate v1.")
    input.fileHandleForWriting.write(frame(2, tag: 12))
    let shutdown = try receiveBody(output, requestID: 2)
    try require(shutdown == Data([1]), "Shutdown was not acknowledged.")
    try? input.fileHandleForWriting.close()
    try require(ProcessTermination.waitForExit(process, timeout: deadline), "Graceful package child did not exit.")
    process.waitUntilExit()
    try require(process.terminationStatus == 0, "Graceful package child returned a nonzero exit status.")
    try? output.fileHandleForReading.close()
    print("PASS graceful Stop/Shutdown child exit (status 0)")
}

private func testMalformedProtocol(_ executable: URL) throws {
    let (process, input, output) = try launch(executable)
    defer { cleanUpDirectChild(process, input: input, output: output) }
    input.fileHandleForWriting.write(Data("bad!".utf8))
    try? input.fileHandleForWriting.close()
    try require(ProcessTermination.waitForExit(process, timeout: deadline), "Malformed-protocol child did not exit.")
    process.waitUntilExit()
    try require(process.terminationStatus != 0, "Malformed protocol unexpectedly succeeded.")
    try? output.fileHandleForReading.close()
    print("PASS protocol-error child exit (status \(process.terminationStatus))")
}

private func waitForProcessToDisappear(_ pid: pid_t, timeout: TimeInterval) -> Bool {
    let end = Date().addingTimeInterval(timeout)
    while Date() < end {
        if kill(pid, 0) != 0 && errno == ESRCH { return true }
        Thread.sleep(forTimeInterval: 0.02)
    }
    return kill(pid, 0) != 0 && errno == ESRCH
}

private func waitForProcessGroupToDisappear(_ group: pid_t, timeout: TimeInterval) -> Bool {
    let end = Date().addingTimeInterval(timeout)
    while Date() < end {
        if kill(-group, 0) != 0 && errno == ESRCH { return true }
        Thread.sleep(forTimeInterval: 0.02)
    }
    return kill(-group, 0) != 0 && errno == ESRCH
}

private func readPIDLine(_ handle: FileHandle) throws -> pid_t {
    let end = Date().addingTimeInterval(deadline)
    var bytes = Data()
    while Date() < end {
        guard let part = try? readExactly(handle, count: 1), let byte = part.first else { break }
        if byte == 10 { break }
        bytes.append(byte)
    }
    guard let text = String(data: bytes, encoding: .utf8), let pid = Int32(text) else {
        throw NSError(domain: "ProcessTerminationTests", code: 7, userInfo: [NSLocalizedDescriptionKey: "Force-close supervisor did not report its child PID."])
    }
    return pid
}

private func runForceKilledParent(_ executable: URL) throws {
    let runner = URL(fileURLWithPath: CommandLine.arguments[0]).standardizedFileURL
    let parent = Process()
    let report = Pipe()
    parent.executableURL = runner
    parent.arguments = ["--parent", executable.path]
    parent.standardOutput = report
    parent.standardError = FileHandle.nullDevice
    try parent.run()
    let groupGuard = TestProcessGroupGuard(group: parent.processIdentifier)
    defer {
        groupGuard.cleanUp()
    }

    let pid: pid_t
    do {
        pid = try readPIDLine(report.fileHandleForReading)
    } catch {
        groupGuard.cleanUp()
        _ = kill(parent.processIdentifier, SIGKILL)
        parent.waitUntilExit()
        try require(
            waitForProcessGroupToDisappear(parent.processIdentifier, timeout: deadline),
            "Could not clean up the force-close process group after supervisor startup failed."
        )
        throw error
    }

    _ = kill(parent.processIdentifier, SIGKILL)
    guard ProcessTermination.waitForExit(parent, timeout: deadline) else {
        groupGuard.cleanUp()
        _ = kill(parent.processIdentifier, SIGKILL)
        parent.waitUntilExit()
        try require(
            waitForProcessGroupToDisappear(parent.processIdentifier, timeout: deadline),
            "Could not clean up the force-close process group after supervisor timeout."
        )
        throw NSError(domain: "ProcessTerminationTests", code: 8, userInfo: [NSLocalizedDescriptionKey: "Force-close supervisor remained live."])
    }
    parent.waitUntilExit()
    try require(parent.terminationReason == .uncaughtSignal && parent.terminationStatus == SIGKILL, "Supervisor was not force-killed.")
    guard waitForProcessToDisappear(pid, timeout: deadline) else {
        groupGuard.cleanUp()
        try require(
            waitForProcessGroupToDisappear(parent.processIdentifier, timeout: deadline),
            "Force-close process group remained after timeout cleanup."
        )
        throw NSError(domain: "ProcessTerminationTests", code: 9, userInfo: [NSLocalizedDescriptionKey: "Package child PID \(pid) survived force-close parent death."])
    }
    groupGuard.cleanUp()
    try require(
        waitForProcessGroupToDisappear(parent.processIdentifier, timeout: deadline),
        "Force-close test watchdog process group remained after cleanup."
    )
    print("PASS force-killed app-like parent; EOF reaped package child PID \(pid)")
}

private func parentMode(_ executable: URL) throws -> Never {
    guard setpgid(0, 0) == 0 else {
        throw NSError(domain: "ProcessTerminationTests", code: 11, userInfo: [NSLocalizedDescriptionKey: "Could not isolate force-close process group."])
    }
    let groupGuard = TestProcessGroupGuard(group: getpid())
    defer { groupGuard.cleanUp() }
    let watchdog = Process()
    watchdog.executableURL = URL(fileURLWithPath: "/bin/sleep")
    watchdog.arguments = ["30"]
    watchdog.standardOutput = FileHandle.nullDevice
    watchdog.standardError = FileHandle.nullDevice
    try watchdog.run()
    let (process, input, output) = try launch(executable)
    _ = input // Retain the parent pipe writer until this supervisor is killed.
    _ = output
    FileHandle.standardOutput.write(Data("\(process.processIdentifier)\n".utf8))
    while true { Thread.sleep(forTimeInterval: 60) }
}

private func testRepeatedLaunchExit(_ executable: URL) throws {
    for cycle in 1...3 {
        let (process, input, output) = try launch(executable)
        defer { cleanUpDirectChild(process, input: input, output: output) }
        input.fileHandleForWriting.write(frame(1, tag: 0, extra: Data([0, 1, 0, 1])))
        _ = try receiveBody(output, requestID: 1)
        input.fileHandleForWriting.write(frame(2, tag: 12))
        _ = try receiveBody(output, requestID: 2)
        try? input.fileHandleForWriting.close()
        try require(ProcessTermination.waitForExit(process, timeout: deadline), "Repeated cycle \(cycle) child did not exit.")
        process.waitUntilExit()
        try require(process.terminationStatus == 0, "Repeated cycle \(cycle) had exit \(process.terminationStatus).")
        try? output.fileHandleForReading.close()
    }
    print("PASS three repeated launch/Stop/Shutdown/exit cycles")
}

@MainActor
private func testShutdownCompletionGate() throws {
    let gate = ShutdownCompletionGate()
    var starts = 0
    var firstCompletion = 0
    var secondCompletion = 0
    var postCompletion = 0
    var reusedCompletion = 0
    var release: (() -> Void)?

    gate.run(completion: { firstCompletion += 1 }) { finish in
        starts += 1
        release = finish
    }
    gate.run(completion: { secondCompletion += 1 }) { _ in
        starts += 100
    }
    try require(starts == 1 && firstCompletion == 0 && secondCompletion == 0,
                "Concurrent shutdown callers started duplicate cleanup or completed early.")
    release?()
    try require(firstCompletion == 1 && secondCompletion == 1,
                "Shutdown gate did not complete every waiting caller exactly once.")

    gate.run(completion: { postCompletion += 1 }) { _ in starts += 1000 }
    try require(postCompletion == 1 && starts == 1,
                "A completed shutdown repeated cleanup instead of replying immediately.")

    gate.prepareForReuse()
    gate.run(completion: { reusedCompletion += 1 }) { finish in
        starts += 1
        release = finish
    }
    try require(starts == 2 && reusedCompletion == 0,
                "A reopened window could not start a fresh cleanup lifecycle.")
    release?()
    try require(reusedCompletion == 1, "Reused shutdown gate did not complete its caller.")
    print("PASS shutdown coordination is idempotent, fans out completions, and resets for reopened windows")
}

private func testProcessDrainHelper() throws {
    let process = Process()
    process.executableURL = URL(fileURLWithPath: "/bin/sh")
    // Preserve SIGTERM as ignored across exec so this child requires the
    // helper's SIGKILL escalation after its graceful deadline.
    process.arguments = ["-c", "trap '' TERM; exec /bin/sleep 30"]
    process.standardOutput = FileHandle.nullDevice
    process.standardError = FileHandle.nullDevice
    try process.run()
    defer {
        if process.isRunning {
            ProcessTermination.terminateAndWait(process, gracefulTimeout: 0.25, forcedTimeout: 1)
        }
        process.waitUntilExit()
    }
    let startedAt = Date()
    ProcessTermination.drainAndReap(process, gracefulTimeout: 1, forcedTimeout: deadline)
    let elapsed = Date().timeIntervalSince(startedAt)
    try require(
        !process.isRunning && process.terminationReason == .uncaughtSignal && process.terminationStatus == SIGKILL,
        "Process drain helper did not escalate and reap a SIGTERM-resistant child."
    )
    try require(elapsed >= 0.99, "Process drain helper skipped the graceful shutdown window.")
    print("PASS process drain helper waits for grace, escalates to SIGKILL, and reaps")
}

@main
private struct ProcessTerminationTests {
    @MainActor static func main() {
        do {
            if CommandLine.arguments.count == 3, CommandLine.arguments[1] == "--parent" {
                try parentMode(URL(fileURLWithPath: CommandLine.arguments[2]).standardizedFileURL)
            }
            guard CommandLine.arguments.count == 2 else {
                throw NSError(domain: "ProcessTerminationTests", code: 10, userInfo: [NSLocalizedDescriptionKey: "Usage: ProcessTerminationTests /path/to/rust-music-cli"])
            }
            let executable = URL(fileURLWithPath: CommandLine.arguments[1]).standardizedFileURL
            try testShutdownCompletionGate()
            try testProcessDrainHelper()
            try testGracefulShutdown(executable)
            try testMalformedProtocol(executable)
            try testRepeatedLaunchExit(executable)
            try runForceKilledParent(executable)
        } catch {
            fputs("FAIL: \(error.localizedDescription)\n", stderr)
            exit(1)
        }
    }
}
