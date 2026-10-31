import Darwin
import Foundation

enum ProcessTermination {
    static func ensureStopped(
        _ process: Process,
        gracefulTimeout: TimeInterval = 2,
        terminationTimeout: TimeInterval = 1
    ) {
        guard process.isRunning else { return }
        guard !waitForExit(process, timeout: gracefulTimeout) else { return }
        process.terminate()
        guard !waitForExit(process, timeout: terminationTimeout) else { return }
        _ = kill(process.processIdentifier, SIGKILL)
        // The registered output-reader task owns waitUntilExit and reaping.
        // Poll here so the app termination barrier cannot finish early.
        while process.isRunning { Thread.sleep(forTimeInterval: 0.01) }
    }

    static func terminateAndWait(
        _ process: Process,
        gracefulTimeout: TimeInterval = 0.5,
        forcedTimeout: TimeInterval = 2
    ) {
        guard process.isRunning else { return }
        process.terminate()
        guard waitForExit(process, timeout: gracefulTimeout) else {
            _ = kill(process.processIdentifier, SIGKILL)
            if !waitForExit(process, timeout: forcedTimeout) {
                // A process cannot ignore SIGKILL; reap it before allowing the
                // application termination barrier to complete.
                process.waitUntilExit()
            }
            return
        }
    }

    static func drainAndReap(
        _ process: Process,
        gracefulTimeout: TimeInterval = 2,
        forcedTimeout: TimeInterval = 2
    ) {
        // The caller has already sent its graceful Stop/Shutdown request. Give
        // that protocol a chance to finish before escalating to termination.
        if process.isRunning && !waitForExit(process, timeout: gracefulTimeout) {
            terminateAndWait(process, gracefulTimeout: 1, forcedTimeout: forcedTimeout)
        }
        // SIGKILL cannot be ignored. Reap rather than letting the owning
        // application exit while its child remains live.
        process.waitUntilExit()
    }

    static func waitForExit(_ process: Process, timeout: TimeInterval) -> Bool {
        let deadline = Date().addingTimeInterval(max(0, timeout))
        while process.isRunning && Date() < deadline {
            Thread.sleep(forTimeInterval: 0.01)
        }
        return !process.isRunning
    }
}

@MainActor
final class ShutdownCompletionGate {
    private enum State {
        case idle
        case shuttingDown
        case shutDown
    }

    private var state = State.idle
    private var completions: [() -> Void] = []

    var acceptsWork: Bool { state == .idle }

    func run(completion: (() -> Void)?, operation: (@escaping () -> Void) -> Void) {
        switch state {
        case .idle:
            state = .shuttingDown
            if let completion { completions.append(completion) }
            operation { [weak self] in self?.finish() }
        case .shuttingDown:
            if let completion { completions.append(completion) }
        case .shutDown:
            completion?()
        }
    }

    func prepareForReuse() {
        guard state == .shutDown else { return }
        state = .idle
    }

    private func finish() {
        guard state == .shuttingDown else { return }
        state = .shutDown
        let callbacks = completions
        completions.removeAll(keepingCapacity: false)
        callbacks.forEach { $0() }
    }
}
