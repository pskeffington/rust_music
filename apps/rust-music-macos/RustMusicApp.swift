import AppKit
import CoreAudio
import Darwin
import SwiftUI
import UniformTypeIdentifiers

private enum RustMusicPalette {
    static let background = Color(red: 0.018, green: 0.030, blue: 0.021)
    static let surface = Color(red: 0.035, green: 0.074, blue: 0.043)
    static let display = Color(red: 0.012, green: 0.043, blue: 0.024)
    static let phosphor = Color(red: 0.63, green: 0.98, blue: 0.47)
    static let muted = Color(red: 0.48, green: 0.70, blue: 0.48)
    static let line = Color(red: 0.18, green: 0.36, blue: 0.21)
    static let brass = Color(red: 0.34, green: 0.27, blue: 0.15)
    static let brassHighlight = Color(red: 0.61, green: 0.49, blue: 0.28)
}

struct QueueEntry: Identifiable, Equatable {
    let index: Int
    let isSelected: Bool
    let relativePath: String
    var id: Int { index }
}

struct UserPlaylist: Identifiable, Equatable {
    let name: String
    let paths: [String]
    var id: String { name }
}

struct AudioOutputChoice: Identifiable, Equatable {
    let uid: String
    let name: String
    let isSystemDefault: Bool
    var id: String { uid }
}

private enum CoreAudioOutputDiscovery {
    private static let maximumDevices = 256
    private static let maximumDeviceTextBytes = 1024

    static func list() throws -> [AudioOutputChoice] {
        var devicesAddress = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDevices,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        var dataSize: UInt32 = 0
        var status = AudioObjectGetPropertyDataSize(
            AudioObjectID(kAudioObjectSystemObject), &devicesAddress, 0, nil, &dataSize
        )
        guard status == noErr else { throw failure("device list", status) }
        let deviceBytes = MemoryLayout<AudioDeviceID>.size
        guard dataSize > 0, dataSize.isMultiple(of: UInt32(deviceBytes)),
              dataSize / UInt32(deviceBytes) <= maximumDevices else {
            return []
        }

        var deviceIDs = [AudioDeviceID](repeating: 0, count: Int(dataSize) / deviceBytes)
        status = deviceIDs.withUnsafeMutableBufferPointer { buffer in
            guard let baseAddress = buffer.baseAddress else { return kAudioHardwareBadDeviceError }
            return AudioObjectGetPropertyData(
                AudioObjectID(kAudioObjectSystemObject),
                &devicesAddress,
                0,
                nil,
                &dataSize,
                UnsafeMutableRawPointer(baseAddress)
            )
        }
        guard status == noErr else { throw failure("device list", status) }
        guard dataSize <= UInt32(deviceIDs.count * deviceBytes), dataSize.isMultiple(of: UInt32(deviceBytes)) else {
            throw RustMusicFailure(message: "CoreAudio returned an invalid output device list.")
        }

        let defaultID = defaultOutputDeviceID()
        var outputs: [AudioOutputChoice] = []
        for deviceID in deviceIDs.prefix(Int(dataSize) / deviceBytes) {
            var streamsAddress = AudioObjectPropertyAddress(
                mSelector: kAudioDevicePropertyStreams,
                mScope: kAudioObjectPropertyScopeOutput,
                mElement: kAudioObjectPropertyElementMain
            )
            var streamBytes: UInt32 = 0
            let streamStatus = AudioObjectGetPropertyDataSize(
                deviceID, &streamsAddress, 0, nil, &streamBytes
            )
            guard streamStatus == noErr, streamBytes >= UInt32(MemoryLayout<AudioStreamID>.size) else {
                continue
            }
            guard let uid = stringProperty(deviceID, selector: kAudioDevicePropertyDeviceUID),
                  let name = stringProperty(deviceID, selector: kAudioObjectPropertyName),
                  !uid.isEmpty, !name.isEmpty,
                  uid.utf8.count <= maximumDeviceTextBytes,
                  name.utf8.count <= maximumDeviceTextBytes,
                  !uid.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) }),
                  !name.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) }) else {
                continue
            }
            outputs.append(AudioOutputChoice(uid: uid, name: name, isSystemDefault: deviceID == defaultID))
        }
        return outputs
    }

    private static func defaultOutputDeviceID() -> AudioDeviceID? {
        var address = AudioObjectPropertyAddress(
            mSelector: kAudioHardwarePropertyDefaultOutputDevice,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        var deviceID: AudioDeviceID = 0
        var size = UInt32(MemoryLayout<AudioDeviceID>.size)
        let status = withUnsafeMutablePointer(to: &deviceID) { pointer in
            AudioObjectGetPropertyData(
                AudioObjectID(kAudioObjectSystemObject),
                &address,
                0,
                nil,
                &size,
                UnsafeMutableRawPointer(pointer)
            )
        }
        return status == noErr && deviceID != 0 ? deviceID : nil
    }

    private static func stringProperty(_ deviceID: AudioDeviceID, selector: AudioObjectPropertySelector) -> String? {
        var address = AudioObjectPropertyAddress(
            mSelector: selector,
            mScope: kAudioObjectPropertyScopeGlobal,
            mElement: kAudioObjectPropertyElementMain
        )
        var result: UnsafeRawPointer?
        var size = UInt32(MemoryLayout<UnsafeRawPointer?>.size)
        let status = withUnsafeMutablePointer(to: &result) { pointer in
            AudioObjectGetPropertyData(
                deviceID,
                &address,
                0,
                nil,
                &size,
                UnsafeMutableRawPointer(pointer)
            )
        }
        guard status == noErr, let result else { return nil }
        return Unmanaged<CFString>.fromOpaque(result).takeRetainedValue() as String
    }

    private static func failure(_ stage: String, _ status: OSStatus) -> RustMusicFailure {
        RustMusicFailure(message: "CoreAudio could not read the \(stage) (OSStatus \(status)).")
    }
}

struct ParsedQueue {
    let entries: [QueueEntry]
    let repeatMode: String
    let shuffleSeed: String?
    let current: Int?
    let candidateAvailable: Bool?
}

private enum PackageRequest {
    case hello
    case activate
    case scanLibrary(String)
    case select(UInt16)
    case previous
    case next
    case play
    case stop
    case pause
    case resumePlayback
    case setVolumePercent(UInt8)
    case seekFrame(UInt64)
    case snapshot(UInt16)
    case shutdown

    func payload(requestID: UInt64) throws -> Data {
        guard requestID > 0 else { throw RustMusicFailure(message: "Package request ID exhausted its protocol range.") }
        var body = Data()
        switch self {
        case .hello:
            body.append(0); body.appendBE(UInt16(1)); body.appendBE(UInt16(1))
        case .activate:
            body.append(1); body.appendBE(UInt16(640)); body.appendBE(UInt16(520))
        case .scanLibrary(let root):
            guard root.utf8.count <= 4096 else { throw RustMusicFailure(message: "Library path is too long for the package protocol.") }
            body.append(5); try body.appendText(root, max: 4096)
        case .select(let index): body.append(6); body.appendBE(index)
        case .previous: body.append(7)
        case .next: body.append(8)
        case .play: body.append(9)
        case .stop: body.append(10)
        case .snapshot(let page): body.append(11); body.appendBE(page)
        case .shutdown: body.append(12)
        case .pause: body.append(13)
        case .resumePlayback: body.append(14)
        case .setVolumePercent(let percent):
            guard percent <= 100 else { throw RustMusicFailure(message: "Volume must be from 0 to 100 percent.") }
            body.append(15); body.append(percent)
        case .seekFrame(let frame):
            body.append(16); body.appendBE(frame)
        }
        var payload = Data([1, 1]) // RBPK v1 request envelope
        payload.appendBE(requestID)
        try payload.appendText("org.rustboy.music", max: 96)
        payload.append(body)
        guard payload.count <= 16_384 else { throw RustMusicFailure(message: "Package request exceeds the frame limit.") }
        return payload
    }
}

private extension Data {
    mutating func appendBE(_ value: UInt16) {
        append(UInt8((value >> 8) & 0xff)); append(UInt8(value & 0xff))
    }
    mutating func appendBE(_ value: UInt32) {
        for shift in stride(from: 24, through: 0, by: -8) {
            append(UInt8((value >> UInt32(shift)) & 0xff))
        }
    }
    mutating func appendBE(_ value: UInt64) {
        for shift in stride(from: 56, through: 0, by: -8) { append(UInt8((value >> UInt64(shift)) & 0xff)) }
    }
    mutating func appendText(_ value: String, max: Int) throws {
        let encoded = Data(value.utf8)
        guard encoded.count <= max, encoded.count <= Int(UInt16.max),
              !value.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) }) else {
            throw RustMusicFailure(message: "Package text is outside its protocol bound.")
        }
        appendBE(UInt16(encoded.count)); append(encoded)
    }
}

private final class PackageProcessClient {
    private let process: Process
    private let input: FileHandle
    private let output: FileHandle
    private let stderrCapture: BoundedChildStderrCapture
    private var requestID: UInt64 = 1

    init(executable: URL, root: URL, outputDeviceUID: String?) throws {
        let process = Process()
        process.executableURL = executable
        process.arguments = ["serve", "--desktop"]
        let inputPipe = Pipe()
        let outputPipe = Pipe()
        let stderrPipe = Pipe()
        process.standardInput = inputPipe
        process.standardOutput = outputPipe
        process.standardError = stderrPipe
        var childEnvironment = process.environment ?? ProcessInfo.processInfo.environment
        childEnvironment["RUST_MUSIC_AUDIO_DIAGNOSTICS"] = "1"
        if let outputDeviceUID {
            childEnvironment["RUST_MUSIC_OUTPUT_DEVICE_UID"] = outputDeviceUID
        } else {
            childEnvironment.removeValue(forKey: "RUST_MUSIC_OUTPUT_DEVICE_UID")
        }
        process.environment = childEnvironment
        try process.run()
        self.process = process
        input = inputPipe.fileHandleForWriting
        output = outputPipe.fileHandleForReading
        stderrCapture = BoundedChildStderrCapture()
        stderrCapture.startReading(from: stderrPipe.fileHandleForReading)
        do {
            guard case .hello(let version) = try request(.hello), version == 1 else {
                throw RustMusicFailure(message: "Rust Music package protocol negotiation failed.")
            }
            if case .failure(let message, _) = try request(.activate) { throw RustMusicFailure(message: message) }
            if case .failure(let message, _) = try request(.scanLibrary(root.path)) { throw RustMusicFailure(message: message) }
        } catch {
            shutdown(force: true)
            throw error
        }
    }

    func request(_ body: PackageRequest) throws -> PackageResponse {
        guard process.isRunning else {
            throw childStreamFailure("Rust Music playback process has exited.")
        }
        if case .play = body {
            // A persistent package process may play multiple tracks. Scope the
            // retained native diagnostic to this session before it can emit a
            // new AudioQueue record, so Stop/EOF cannot reuse prior-track data.
            stderrCapture.resetDiagnosticCapture()
        }
        let timeoutSeconds: TimeInterval
        if case .scanLibrary = body { timeoutSeconds = 30 } else { timeoutSeconds = 5 }
        let watchdog = DispatchWorkItem { [process] in
            guard process.isRunning else { return }
            process.terminate()
            DispatchQueue.global().asyncAfter(deadline: .now() + 2) {
                if process.isRunning { _ = kill(process.processIdentifier, SIGKILL) }
            }
        }
        DispatchQueue.global().asyncAfter(deadline: .now() + timeoutSeconds, execute: watchdog)
        defer { watchdog.cancel() }
        let id = requestID
        requestID &+= 1
        let payload = try body.payload(requestID: id)
        var frame = Data([0x52, 0x42, 0x50, 0x4b])
        frame.appendBE(UInt32(payload.count))
        frame.append(payload)
        do {
            try input.write(contentsOf: frame)
        } catch {
            if !process.isRunning {
                throw childStreamFailure("Rust Music playback process stopped while receiving a request.")
            }
            throw error
        }
        let header: Data
        do {
            header = try readExact(8)
        } catch is PackageStreamClosed {
            throw childStreamFailure("Rust Music playback process closed its package stream.")
        }
        guard Array(header.prefix(4)) == [0x52, 0x42, 0x50, 0x4b] else {
            throw RustMusicFailure(message: "Rust Music returned an invalid package frame header.")
        }
        var lengthReader = PackageReader(bytes: Array(header.dropFirst(4)))
        let length = Int(try lengthReader.u32())
        guard (1...16_384).contains(length) else { throw RustMusicFailure(message: "Rust Music package frame length is invalid.") }
        let replyBytes: Data
        do {
            replyBytes = try readExact(length)
        } catch is PackageStreamClosed {
            throw childStreamFailure("Rust Music playback process closed its package stream.")
        }
        var reader = PackageReader(bytes: Array(replyBytes))
        guard try reader.byte() == 1, try reader.byte() == 2,
              try reader.u64() == id,
              try reader.text(max: 96) == "org.rustboy.music" else {
            throw RustMusicFailure(message: "Rust Music package reply identity or sequence is invalid.")
        }
        let reply: PackageResponse
        switch try reader.byte() {
        case 0:
            reply = .hello(try reader.u16())
        case 1:
            reply = .ack
        case 2:
            let snapshot = try PackagePlaybackSnapshot.decode(from: &reader)
            let isStopRequest: Bool
            if case .stop = body { isStopRequest = true } else { isStopRequest = false }
            // The stderr reader retains the most recent completed AudioQueue
            // record for Stop/EOF diagnostics. Never attach that old teardown
            // record to an active snapshot from a later playback session.
            let diagnostic: String?
            if isStopRequest || [3, 4, 5].contains(snapshot.state) {
                diagnostic = stderrCapture.statusAfterTeardown(timeout: 0.2)
            } else {
                diagnostic = nil
            }
            reply = .view(snapshot, diagnostic: diagnostic)
        case 3:
            _ = try reader.byte() // error code
            let message = try reader.text(max: 256)
            reply = .failure(message, diagnostic: stderrCapture.statusAfterTeardown(timeout: 0.2))
        default:
            throw RustMusicFailure(message: "Rust Music returned an unknown package reply.")
        }
        guard reader.offset == reader.bytes.count else { throw RustMusicFailure(message: "Rust Music package reply has trailing data.") }
        return reply
    }

    func shutdown(force: Bool = false) {
        if !force, process.isRunning {
            _ = try? request(.stop)
            _ = try? request(.shutdown)
        }
        try? input.close()
        ProcessTermination.drainAndReap(process, gracefulTimeout: 2, forcedTimeout: 1)
        stderrCapture.waitForDrain(timeout: 1)
    }

    private func readExact(_ count: Int) throws -> Data {
        var data = Data()
        data.reserveCapacity(count)
        while data.count < count {
            guard let chunk = try output.read(upToCount: count - data.count), !chunk.isEmpty else {
                throw PackageStreamClosed()
            }
            data.append(chunk)
        }
        return data
    }

    private func childStreamFailure(_ message: String) -> RustMusicFailure {
        // Allow stderr already written by a just-exited child to reach the
        // bounded reader. Its contents, including paths, are never surfaced.
        if !process.isRunning { stderrCapture.waitForDrain(timeout: 0.1) }
        let retainedBytes = stderrCapture.retainedByteCount
        return RustMusicFailure(
            message: "\(message) Child stderr retained \(retainedBytes) bytes (maximum 4096); content withheld."
        )
    }
}

private struct PackageStreamClosed: Error {}

/// Continuously drains child stderr so it cannot block the RBPK server, while
/// retaining at most 4 KiB for a byte-count-only failure diagnostic.
private final class BoundedChildStderrCapture {
    fileprivate static let maximumRetainedBytes = 4 * 1024
    private let lock = NSLock()
    private let readerGroup = DispatchGroup()
    private let readerQueue = DispatchQueue(label: "org.rustboy.music.stderr-reader", qos: .utility)
    private var retained = Data()
    private var readSource: DispatchSourceRead?

    var retainedByteCount: Int {
        lock.lock()
        defer { lock.unlock() }
        return retained.count
    }

    var diagnosticSummary: AudioQueueDiagnosticRecord? {
        lock.lock()
        let bytes = retained
        lock.unlock()
        return AudioQueueDiagnosticRecord.parse(bytes)
    }

    func resetDiagnosticCapture() {
        lock.lock()
        retained.removeAll(keepingCapacity: true)
        lock.unlock()
    }

    func startReading(from handle: FileHandle) {
        readerGroup.enter()
        let descriptor = handle.fileDescriptor
        let source = DispatchSource.makeReadSource(fileDescriptor: descriptor, queue: readerQueue)
        readSource = source
        source.setEventHandler { [weak self] in
            var bytes = [UInt8](repeating: 0, count: 1024)
            let amount = bytes.withUnsafeMutableBytes { buffer in
                Darwin.read(descriptor, buffer.baseAddress, buffer.count)
            }
            if amount > 0, let self {
                self.append(Data(bytes.prefix(amount)))
            } else if amount == 0 || (amount < 0 && errno != EAGAIN && errno != EINTR) {
                source.cancel()
            }
        }
        source.setCancelHandler { [self, handle] in
            try? handle.close()
            readerGroup.leave()
        }
        source.resume()
    }

    private func append(_ bytes: Data) {
        lock.lock()
        retained.append(bytes)
        if retained.count > Self.maximumRetainedBytes {
            retained.removeSubrange(0..<(retained.count - Self.maximumRetainedBytes))
        }
        lock.unlock()
    }

    func waitForDrain(timeout: TimeInterval) {
        _ = readerGroup.wait(timeout: .now() + timeout)
    }

    func waitForDiagnostic(timeout: TimeInterval) {
        let deadline = Date().addingTimeInterval(max(0, timeout))
        while Date() < deadline {
            if diagnosticSummary != nil { return }
            Thread.sleep(forTimeInterval: 0.01)
        }
    }

    func statusAfterTeardown(timeout: TimeInterval) -> String {
        waitForDiagnostic(timeout: timeout)
        if let summary = diagnosticSummary { return summary.statusText }
        return "AudioQueue diagnostic record unavailable; stderr retained \(retainedByteCount) bytes."
    }
}

/// Whitelisted numeric fields from the child's fixed diagnostics record.
/// Raw stderr never escapes `BoundedChildStderrCapture`.
private struct AudioQueueDiagnosticRecord {
    let rate: UInt32
    let bits: UInt8
    let channels: UInt8
    let created: UInt64
    let createStatus: StatusCode
    let routeStatus: StatusCode
    let routeDeviceListStatus: StatusCode
    let routeDefaultListed: String
    let routeTargetListed: String
    let allocated: UInt64
    let capacityBytes: UInt64
    let allocationStatus: StatusCode
    let enqueued: UInt64
    let frames: UInt64
    let pcmBlocks: UInt64
    let firstEnqueueMicros: String
    let firstCallbackObservedMicros: String
    let writeTimeouts: UInt64
    let enqueueStatus: StatusCode
    let startFailures: UInt64
    let callbackGapSamples: UInt64
    let callbackGapMaxMicros: UInt64
    let queueDepthSamples: UInt64
    let queueEmptyWrites: UInt64
    let queueDepthMax: UInt64
    let nonzeroSamples: UInt64
    let peakSample: UInt64
    let starts: UInt64
    let startStatus: StatusCode
    let deviceStatus: StatusCode
    let deviceAssociated: String
    let deviceMatchesDefault: String
    let deviceMatchesTarget: String
    let deviceListStatus: StatusCode
    let deviceListed: String
    let outputMeterEnableStatus: StatusCode
    let outputMeterStatus: StatusCode
    let outputMeterDBStatus: StatusCode
    let outputMeterPeakMillionths: String
    let outputMeterPeakMillidb: String
    let outputMeterObservations: UInt64
    let queueRunningStatus: StatusCode
    let queueIsRunning: String
    let queueFormatText: String
    let tapCreationStatus: StatusCode
    let tapCallbacks: UInt64
    let tapFrames: UInt64
    let tapNonzeroBytes: UInt64
    let tapDataBuffers: UInt64
    let enqueuedNonzeroBytes: UInt64
    let callbacks: UInt64
    let teardown: UInt64
    let stopStatus: StatusCode
    let drained: String
    let immediateStopStatus: StatusCode
    let disposeStatus: StatusCode

    enum StatusCode {
        case unavailable
        case value(Int32)
    }

    private static let prefix = "RUST_MUSIC_AUDIO_DIAG "
    private static let expectedKeys: Set<String> = [
        "rate", "bits", "channels", "created", "create_status", "route_status",
        "route_device_list_status", "route_default_listed", "route_target_listed", "allocated",
        "capacity_bytes", "allocation_status", "pcm_blocks", "first_enqueue_us", "first_callback_observed_us", "write_timeouts", "enqueue_status", "start_failures", "callback_gap_samples", "callback_gap_max_us", "queue_depth_samples", "queue_empty_writes", "queue_depth_max", "enqueued", "frames", "nonzero_samples", "peak_sample", "starts",
        "start_status", "device_status", "device_associated", "device_default", "device_target",
        "device_list_status", "device_listed", "output_meter_enable_status", "output_meter_query_status",
        "output_meter_db_status", "output_meter_peak_millionths", "output_meter_peak_millidb",
        "output_meter_observations", "queue_running_status", "queue_is_running",
        "stream_status", "stream_matches", "stream_rate", "stream_flags", "stream_bytes_frame",
        "stream_channels", "stream_bits", "device_rate_status", "device_rate",
        "device_channels_status", "device_channels", "converter_error_status", "converter_error",
        "tap_status", "tap_callbacks", "tap_frames", "tap_nonzero_bytes", "tap_data_buffers",
        "tap_rate", "tap_format", "tap_flags", "tap_bytes_packet", "tap_frames_packet",
        "tap_bytes_frame", "tap_channels", "tap_bits",
        "queue_volume_set_status", "queue_volume_get_status", "queue_volume_millionths",
        "enqueued_nonzero_bytes",
        "callbacks", "teardown", "stop_status", "drained",
        "immediate_stop_status", "dispose_status"
    ]

    static func parse(_ bytes: Data) -> AudioQueueDiagnosticRecord? {
        guard bytes.count <= BoundedChildStderrCapture.maximumRetainedBytes else { return nil }
        let text = String(decoding: bytes, as: UTF8.self)
        for line in text.split(whereSeparator: \.isNewline).reversed() {
            guard line.hasPrefix(prefix), line.utf8.count <= 2048 else { continue }
            let tokens = line.dropFirst(prefix.count).split(separator: " ")
            guard tokens.count == expectedKeys.count else { continue }
            var values: [String: String] = [:]
            var valid = true
            for token in tokens {
                let pair = token.split(separator: "=", maxSplits: 1, omittingEmptySubsequences: false)
                guard pair.count == 2 else { valid = false; break }
                let key = String(pair[0])
                guard expectedKeys.contains(key), values[key] == nil else { valid = false; break }
                values[key] = String(pair[1])
            }
            guard valid, values.count == expectedKeys.count,
                  let rate = values["rate"].flatMap(UInt32.init), rate > 0,
                  let bits = values["bits"].flatMap(UInt8.init), [16, 24, 32].contains(bits),
                  let channels = values["channels"].flatMap(UInt8.init), (1...8).contains(channels),
                  let created = values["created"].flatMap(UInt64.init),
                  let allocated = values["allocated"].flatMap(UInt64.init),
                  let capacityBytes = values["capacity_bytes"].flatMap(UInt64.init),
                  let enqueued = values["enqueued"].flatMap(UInt64.init),
                  let frames = values["frames"].flatMap(UInt64.init),
                  let pcmBlocks = values["pcm_blocks"].flatMap(UInt64.init),
                  let firstEnqueueMicros = optionalUnsigned64(values["first_enqueue_us"]),
                  let firstCallbackObservedMicros = optionalUnsigned64(values["first_callback_observed_us"]),
                  let writeTimeouts = values["write_timeouts"].flatMap(UInt64.init),
                  let enqueueStatus = optionalStatus(values["enqueue_status"]),
                  let startFailures = values["start_failures"].flatMap(UInt64.init),
                  let callbackGapSamples = values["callback_gap_samples"].flatMap(UInt64.init),
                  let callbackGapMaxMicros = values["callback_gap_max_us"].flatMap(UInt64.init),
                  let queueDepthSamples = values["queue_depth_samples"].flatMap(UInt64.init),
                  let queueEmptyWrites = values["queue_empty_writes"].flatMap(UInt64.init),
                  let queueDepthMax = values["queue_depth_max"].flatMap(UInt64.init),
                  let nonzeroSamples = values["nonzero_samples"].flatMap(UInt64.init),
                  let peakSample = values["peak_sample"].flatMap(UInt64.init),
                  let starts = values["starts"].flatMap(UInt64.init),
                  let callbacks = values["callbacks"].flatMap(UInt64.init),
                  let teardown = values["teardown"].flatMap(UInt64.init),
                  let createStatus = optionalStatus(values["create_status"]),
                  let routeStatus = optionalStatus(values["route_status"]),
                  let routeDeviceListStatus = optionalStatus(values["route_device_list_status"]),
                  let routeDefaultListed = values["route_default_listed"],
                  ["0", "1", "none"].contains(routeDefaultListed),
                  let routeTargetListed = values["route_target_listed"],
                  ["0", "1", "none"].contains(routeTargetListed),
                  let allocationStatus = optionalStatus(values["allocation_status"]),
                  let startStatus = optionalStatus(values["start_status"]),
                  let deviceStatus = optionalStatus(values["device_status"]),
                  let deviceAssociated = values["device_associated"],
                  ["0", "1", "none"].contains(deviceAssociated),
                  let deviceMatchesDefault = values["device_default"],
                  ["0", "1", "none"].contains(deviceMatchesDefault),
                  let deviceMatchesTarget = values["device_target"],
                  ["0", "1", "none"].contains(deviceMatchesTarget),
                  let deviceListStatus = optionalStatus(values["device_list_status"]),
                  let deviceListed = values["device_listed"],
                  ["0", "1", "none"].contains(deviceListed),
                  let outputMeterEnableStatus = optionalStatus(values["output_meter_enable_status"]),
                  let outputMeterStatus = optionalStatus(values["output_meter_query_status"]),
                  let outputMeterDBStatus = optionalStatus(values["output_meter_db_status"]),
                  let outputMeterPeakMillionths = values["output_meter_peak_millionths"],
                  outputMeterPeakMillionths == "none" || UInt32(outputMeterPeakMillionths) != nil,
                  let outputMeterPeakMillidb = values["output_meter_peak_millidb"],
                  outputMeterPeakMillidb == "none" || Int32(outputMeterPeakMillidb) != nil,
                  let outputMeterObservations = values["output_meter_observations"].flatMap(UInt64.init),
                  let queueRunningStatus = optionalStatus(values["queue_running_status"]),
                  let queueIsRunning = values["queue_is_running"],
                  ["0", "1", "none"].contains(queueIsRunning),
                  let streamStatus = optionalStatus(values["stream_status"]),
                  let streamMatches = values["stream_matches"], ["0", "1", "none"].contains(streamMatches),
                  let streamRate = optionalUnsigned(values["stream_rate"]),
                  let streamFlags = optionalUnsigned(values["stream_flags"]),
                  let streamBytes = optionalUnsigned(values["stream_bytes_frame"]),
                  let streamChannels = optionalUnsigned(values["stream_channels"]),
                  let streamBits = optionalUnsigned(values["stream_bits"]),
                  let deviceRateStatus = optionalStatus(values["device_rate_status"]),
                  let deviceRate = optionalUnsigned(values["device_rate"]),
                  let deviceChannelsStatus = optionalStatus(values["device_channels_status"]),
                  let deviceChannels = optionalUnsigned(values["device_channels"]),
                  let converterStatus = optionalStatus(values["converter_error_status"]),
                  let converterError = optionalUnsigned(values["converter_error"]),
                  let tapCreationStatus = optionalStatus(values["tap_status"]),
                  let tapCallbacks = values["tap_callbacks"].flatMap(UInt64.init),
                  let tapFrames = values["tap_frames"].flatMap(UInt64.init),
                  let tapNonzeroBytes = values["tap_nonzero_bytes"].flatMap(UInt64.init),
                  let tapDataBuffers = values["tap_data_buffers"].flatMap(UInt64.init),
                  let tapRate = optionalUnsigned(values["tap_rate"]),
                  let tapFormat = optionalUnsigned(values["tap_format"]),
                  let tapFlags = optionalUnsigned(values["tap_flags"]),
                  let tapBytesPacket = optionalUnsigned(values["tap_bytes_packet"]),
                  let tapFramesPacket = optionalUnsigned(values["tap_frames_packet"]),
                  let tapBytesFrame = optionalUnsigned(values["tap_bytes_frame"]),
                  let tapChannels = optionalUnsigned(values["tap_channels"]),
                  let tapBits = optionalUnsigned(values["tap_bits"]),
                  let queueVolumeSetStatus = optionalStatus(values["queue_volume_set_status"]),
                  let queueVolumeGetStatus = optionalStatus(values["queue_volume_get_status"]),
                  let queueVolume = optionalUnsigned(values["queue_volume_millionths"]),
                  let enqueuedNonzeroBytes = values["enqueued_nonzero_bytes"].flatMap(UInt64.init),
                  let stopStatus = optionalStatus(values["stop_status"]),
                  let immediateStopStatus = optionalStatus(values["immediate_stop_status"]),
                  let disposeStatus = optionalStatus(values["dispose_status"]),
                  let drained = values["drained"], ["0", "1", "none"].contains(drained) else {
                continue
            }
            return AudioQueueDiagnosticRecord(
                rate: rate, bits: bits, channels: channels, created: created,
                createStatus: createStatus, routeStatus: routeStatus,
                routeDeviceListStatus: routeDeviceListStatus,
                routeDefaultListed: routeDefaultListed,
                routeTargetListed: routeTargetListed,
                allocated: allocated, capacityBytes: capacityBytes,
                allocationStatus: allocationStatus, enqueued: enqueued, frames: frames,
                pcmBlocks: pcmBlocks, firstEnqueueMicros: firstEnqueueMicros,
                firstCallbackObservedMicros: firstCallbackObservedMicros,
                writeTimeouts: writeTimeouts,
                enqueueStatus: enqueueStatus, startFailures: startFailures,
                callbackGapSamples: callbackGapSamples,
                callbackGapMaxMicros: callbackGapMaxMicros,
                queueDepthSamples: queueDepthSamples,
                queueEmptyWrites: queueEmptyWrites,
                queueDepthMax: queueDepthMax,
                nonzeroSamples: nonzeroSamples, peakSample: peakSample,
                starts: starts, startStatus: startStatus, deviceStatus: deviceStatus,
                deviceAssociated: deviceAssociated,
                deviceMatchesDefault: deviceMatchesDefault,
                deviceMatchesTarget: deviceMatchesTarget,
                deviceListStatus: deviceListStatus, deviceListed: deviceListed,
                outputMeterEnableStatus: outputMeterEnableStatus,
                outputMeterStatus: outputMeterStatus,
                outputMeterDBStatus: outputMeterDBStatus,
                outputMeterPeakMillionths: outputMeterPeakMillionths,
                outputMeterPeakMillidb: outputMeterPeakMillidb,
                outputMeterObservations: outputMeterObservations,
                queueRunningStatus: queueRunningStatus,
                queueIsRunning: queueIsRunning,
                queueFormatText: "stream \(Self.formatStatus(streamStatus))/\(streamMatches) \(streamRate)Hz flags \(streamFlags) bytes/frame \(streamBytes) channels \(streamChannels) bits \(streamBits); device rate \(Self.formatStatus(deviceRateStatus))/\(deviceRate)Hz channels \(Self.formatStatus(deviceChannelsStatus))/\(deviceChannels); converter \(Self.formatStatus(converterStatus))/\(converterError); queue volume set/get \(Self.formatStatus(queueVolumeSetStatus))/\(Self.formatStatus(queueVolumeGetStatus)) = \(queueVolume); pre-effects tap \(Self.formatStatus(tapCreationStatus)) \(tapRate)Hz fmt \(tapFormat) flags \(tapFlags) b/packet \(tapBytesPacket) f/packet \(tapFramesPacket) b/frame \(tapBytesFrame) ch \(tapChannels) bits \(tapBits); callbacks \(tapCallbacks), frames \(tapFrames), nonzero bytes \(tapNonzeroBytes), buffers \(tapDataBuffers); enqueued nonzero bytes \(enqueuedNonzeroBytes)",
                tapCreationStatus: tapCreationStatus,
                tapCallbacks: tapCallbacks,
                tapFrames: tapFrames,
                tapNonzeroBytes: tapNonzeroBytes,
                tapDataBuffers: tapDataBuffers,
                enqueuedNonzeroBytes: enqueuedNonzeroBytes,
                callbacks: callbacks,
                teardown: teardown, stopStatus: stopStatus, drained: drained,
                immediateStopStatus: immediateStopStatus, disposeStatus: disposeStatus
            )
        }
        return nil
    }

    private static func optionalStatus(_ value: String?) -> StatusCode? {
        guard let value else { return nil }
        if value == "none" { return .unavailable }
        guard let status = Int32(value) else { return nil }
        return .value(status)
    }

    private static func formatStatus(_ value: StatusCode) -> String {
        switch value {
        case .unavailable: "none"
        case .value(let status): String(status)
        }
    }

    private static func optionalUnsigned(_ value: String?) -> String? {
        guard let value else { return nil }
        if value == "none" { return value }
        guard UInt32(value) != nil else { return nil }
        return value
    }

    private static func optionalUnsigned64(_ value: String?) -> String? {
        guard let value else { return nil }
        if value == "none" { return value }
        guard UInt64(value) != nil else { return nil }
        return value
    }

    var statusText: String {
        let drainText = drained == "none" ? "unknown" : (drained == "1" ? "complete" : "incomplete")
        let deviceText = deviceAssociated == "1" ? "yes" : (deviceAssociated == "0" ? "no" : "unknown")
        let defaultText = deviceMatchesDefault == "1" ? "yes" : (deviceMatchesDefault == "0" ? "no" : "unknown")
        let targetText = deviceMatchesTarget == "1" ? "yes" : (deviceMatchesTarget == "0" ? "no" : "unknown")
        let runningText = queueIsRunning == "1" ? "yes" : (queueIsRunning == "0" ? "no" : "unknown")
        return "AQ \(rate)Hz/\(bits)b/\(channels)ch create \(created):\(statusValue(createStatus)) route \(statusValue(routeStatus)) list \(statusValue(routeDeviceListStatus)) default/target \(listedText(routeDefaultListed))/\(listedText(routeTargetListed))\nalloc \(allocated)/\(capacityBytes)B PCM blocks \(pcmBlocks), first enqueue \(firstEnqueueMicros)µs, callback observed \(firstCallbackObservedMicros)µs, observed callback gaps \(callbackGapSamples), max \(callbackGapMaxMicros)µs; queue depth samples \(queueDepthSamples), empty writes \(queueEmptyWrites), max depth \(queueDepthMax); write timeouts \(writeTimeouts), enqueue status \(statusValue(enqueueStatus)), start failures \(startFailures); enq \(enqueued)/\(frames)f start \(starts):\(statusValue(startStatus)) device \(statusValue(deviceStatus))/\(deviceText) default \(defaultText) target \(targetText)\nPCM nonzero \(nonzeroSamples) peak \(peakSample) meter enable/query/db \(statusValue(outputMeterEnableStatus))/\(statusValue(outputMeterStatus))/\(statusValue(outputMeterDBStatus)) max \(outputMeterPeakMillionths)/1e6 \(outputMeterPeakMillidb)mdB over \(outputMeterObservations) samples; running \(statusValue(queueRunningStatus))/\(runningText)\n\(queueFormatText)\ncurrent-list \(statusValue(deviceListStatus))/\(listedText(deviceListed)) cb \(callbacks) teardown \(teardown) stop \(statusValue(stopStatus)) drain \(drainText) dispose \(statusValue(disposeStatus)) · audible unconfirmed"
    }

    private func listedText(_ value: String) -> String {
        switch value {
        case "0": "no"
        case "1": "yes"
        default: "unknown"
        }
    }

    private func statusValue(_ value: StatusCode) -> String {
        switch value {
        case .unavailable: "none"
        case .value(let status): String(status)
        }
    }
}

private enum PackageResponse {
    case hello(UInt16)
    case ack
    case view(PackagePlaybackSnapshot, diagnostic: String?)
    case failure(String, diagnostic: String?)
}

@main
struct RustMusicApp: App {
    @NSApplicationDelegateAdaptor(RustMusicApplicationDelegate.self) private var appDelegate
    @StateObject private var model = PlayerModel()

    var body: some Scene {
        Window("Rust Music — Player", id: "player") {
            playerWindow(.player)
        }
        .defaultSize(width: 620, height: 380)
        .windowResizability(.automatic)
        .commands {
            RustMusicCommands(model: model)
        }
        Window("Rust Music — Playlist Editor", id: "playlist") {
            playerWindow(.playlist)
        }
        .defaultSize(width: 720, height: 560)
        .windowResizability(.automatic)
        Window("Rust Music — Media Library", id: "library") {
            playerWindow(.library)
        }
        .defaultSize(width: 1000, height: 700)
        .windowResizability(.automatic)
        Window("Rust Music — Equalizer", id: "equalizer") {
            EqualizerWindow()
                .frame(minWidth: 480, minHeight: 200)
                .background(WindowFramePersistence(name: OperatorWindowLayout.equalizer))
                .background(RustMusicPalette.background)
                .preferredColorScheme(.dark)
                .tint(RustMusicPalette.phosphor)
        }
        .defaultSize(width: 520, height: 300)
        .windowResizability(.automatic)
    }

    private func playerWindow(_ kind: PlayerWindowKind) -> some View {
        PlayerView(model: model, kind: kind)
            .frame(minWidth: kind.minimumWidth, minHeight: kind.minimumHeight)
            .background(WindowFramePersistence(name: kind.frameName))
            .background(RustMusicPalette.background)
            .preferredColorScheme(.dark)
            .tint(RustMusicPalette.phosphor)
    }
}

private struct RustMusicCommands: Commands {
    @ObservedObject var model: PlayerModel
    @Environment(\.openWindow) private var openWindow
    @AppStorage("RustMusic.windowDockingEnabled") private var windowDockingEnabled = true

    var body: some Commands {
        CommandGroup(after: .newItem) {
            Button("Load Music Library…") { model.chooseLibrary() }
                .keyboardShortcut("o", modifiers: [.command])
        }
        CommandGroup(after: .windowArrangement) {
            Button("Show Player") { openWindow(id: "player") }
            Button("Show Playlist Editor") { openWindow(id: "playlist") }
            Button("Show Media Library") { openWindow(id: "library") }
            Button("Show Equalizer") { openWindow(id: "equalizer") }
            Button("Show All Operator Windows") { showAllWindows() }
            Button("Arrange Operator Windows") { arrangeWindows() }
            Toggle("Magnetic Window Docking", isOn: $windowDockingEnabled)
        }
        CommandMenu("Playback") {
            Button("Play / Pause") {
                if model.isPlaying || model.isPaused { model.togglePause() } else { model.play() }
            }
            .keyboardShortcut(.space, modifiers: [])
            Button("Stop") { model.stop() }
                .keyboardShortcut(".", modifiers: [.command])
            Button("Previous Track") { model.previous() }
                .keyboardShortcut(.leftArrow, modifiers: [.command])
            Button("Next Track") { model.next() }
                .keyboardShortcut(.rightArrow, modifiers: [.command])
        }
    }

    private func showAllWindows() {
        openWindow(id: "player")
        openWindow(id: "playlist")
        openWindow(id: "library")
        openWindow(id: "equalizer")
    }

    private func arrangeWindows() {
        showAllWindows()
        OperatorWindowDocking.shared.requestArrangement()
    }
}

private enum PlayerWindowKind {
    case player
    case playlist
    case library

    var minimumWidth: CGFloat {
        switch self {
        case .player: 520
        case .playlist: 520
        case .library: 680
        }
    }

    var minimumHeight: CGFloat {
        switch self {
        case .player: 200
        case .playlist: 200
        case .library: 420
        }
    }

    var frameName: String {
        switch self {
        case .player: OperatorWindowLayout.player
        case .playlist: OperatorWindowLayout.playlist
        case .library: OperatorWindowLayout.library
        }
    }

}

private struct WindowSwitcher: View {
    @Environment(\.openWindow) private var openWindow
    @AppStorage("RustMusic.windowDockingEnabled") private var windowDockingEnabled = true

    var body: some View {
        Menu("WINDOWS") {
            Button("Show All Operator Windows") {
                showAllWindows()
            }
            Button("Arrange Operator Windows") { arrangeWindows() }
            Divider()
            Toggle("Magnetic Docking", isOn: $windowDockingEnabled)
            Divider()
            Button("Player") { openWindow(id: "player") }
            Button("Playlist Editor") { openWindow(id: "playlist") }
            Button("Media Library") { openWindow(id: "library") }
            Button("Equalizer") { openWindow(id: "equalizer") }
        }
        .buttonStyle(RustMusicButtonStyle())
        .accessibilityLabel("Show or focus an operator window")
    }

    private func showAllWindows() {
        openWindow(id: "player")
        openWindow(id: "playlist")
        openWindow(id: "library")
        openWindow(id: "equalizer")
    }

    private func arrangeWindows() {
        showAllWindows()
        OperatorWindowDocking.shared.requestArrangement()
    }
}

private struct WindowFramePersistence: NSViewRepresentable {
    let name: String

    func makeNSView(context: Context) -> NSView {
        WindowFrameObserver(name: name)
    }

    func updateNSView(_ nsView: NSView, context: Context) {}
}

private final class WindowFrameObserver: NSView {
    private let name: String
    private var configured = false

    init(name: String) {
        self.name = name
        super.init(frame: .zero)
    }

    required init?(coder: NSCoder) {
        fatalError("init(coder:) is unavailable")
    }

    override func viewDidMoveToWindow() {
        super.viewDidMoveToWindow()
        guard !configured, let window else { return }
        configured = true
        let restored = window.setFrameUsingName(name)
        _ = window.setFrameAutosaveName(name)
        if !restored {
            let visible = (window.screen ?? NSScreen.main)?.visibleFrame ?? window.frame
            let initial = OperatorWindowLayout.initialFrames(
                in: visible,
                libraryWidth: window.frame.width
            )
            if let frame = initial[name] {
                window.setFrame(frame, display: true)
            }
        }
        OperatorWindowDocking.shared.register(window, name: name)
    }
}

@MainActor
private final class OperatorWindowDocking: NSObject {
    static let shared = OperatorWindowDocking()

    private final class WeakWindow {
        weak var value: NSWindow?

        init(_ value: NSWindow) {
            self.value = value
        }
    }

    private var windows: [String: WeakWindow] = [:]
    private var arrangementPending = false
    private var changingFrames = false

    private override init() {
        super.init()
        UserDefaults.standard.register(defaults: ["RustMusic.windowDockingEnabled": true])
        NotificationCenter.default.addObserver(
            self,
            selector: #selector(windowDidMove(_:)),
            name: NSWindow.didMoveNotification,
            object: nil
        )
    }

    func register(_ window: NSWindow, name: String) {
        windows[name] = WeakWindow(window)
        if arrangementPending {
            arrangeIfReady()
        }
    }

    func requestArrangement() {
        arrangementPending = true
        arrangeIfReady()
    }

    private func arrangeIfReady() {
        guard OperatorWindowLayout.windowOrder.allSatisfy({ windows[$0]?.value != nil }),
              let screen = NSScreen.main ?? windows.values.compactMap(\.value).first?.screen,
              let libraryWindow = windows[OperatorWindowLayout.library]?.value else {
            return
        }
        arrangementPending = false
        let frames = OperatorWindowLayout.initialFrames(
            in: screen.visibleFrame,
            libraryWidth: libraryWindow.frame.width
        )
        changingFrames = true
        for name in OperatorWindowLayout.windowOrder {
            guard let window = windows[name]?.value, let frame = frames[name] else { continue }
            window.setFrame(frame, display: true, animate: true)
        }
        changingFrames = false
        windows[OperatorWindowLayout.player]?.value?.makeKeyAndOrderFront(nil)
    }

    @objc private func windowDidMove(_ notification: Notification) {
        guard !changingFrames,
              UserDefaults.standard.bool(forKey: "RustMusic.windowDockingEnabled"),
              let moved = notification.object as? NSWindow,
              windows.values.contains(where: { $0.value === moved }),
              moved.isVisible,
              !moved.isMiniaturized else {
            return
        }
        let peers = windows.values.compactMap(\.value).filter {
            $0 !== moved && $0.isVisible && !$0.isMiniaturized && $0.screen === moved.screen
        }
        let frame = OperatorWindowLayout.snappedFrame(
            moved.frame,
            to: peers.map(\.frame)
        )
        guard frame.origin != moved.frame.origin else { return }
        changingFrames = true
        moved.setFrame(frame, display: true)
        changingFrames = false
    }
}

private struct EqualizerWindow: View {
    var body: some View {
        PipBoyChassis {
            VStack(alignment: .leading, spacing: 12) {
                HStack {
                    VStack(alignment: .leading, spacing: 3) {
                        Text("EQUALIZER")
                            .font(.system(size: 19, weight: .bold, design: .monospaced))
                            .tracking(1.1)
                        Text("10 BAND · UNAVAILABLE")
                            .font(.system(size: 10, weight: .medium, design: .monospaced))
                            .foregroundStyle(RustMusicPalette.muted)
                    }
                    Spacer()
                    WindowSwitcher()
                }
                Text("No verified DSP processor is connected. Equalizer controls remain disabled.")
                    .font(.system(size: 12, weight: .regular, design: .monospaced))
                    .foregroundStyle(RustMusicPalette.muted)
                    .fixedSize(horizontal: false, vertical: true)
                ForEach(["31 Hz", "62 Hz", "125 Hz", "250 Hz", "500 Hz", "1 kHz", "2 kHz", "4 kHz", "8 kHz", "16 kHz"], id: \.self) { band in
                    HStack(spacing: 10) {
                        Text(band)
                            .font(.system(size: 10, weight: .medium, design: .monospaced))
                            .frame(width: 52, alignment: .trailing)
                        Slider(value: .constant(0.5), in: 0...1)
                            .disabled(true)
                            .accessibilityLabel("\(band) equalizer band unavailable")
                        Text("OFF")
                            .font(.system(size: 9, weight: .medium, design: .monospaced))
                            .foregroundStyle(RustMusicPalette.muted)
                    }
                }
            }
            .padding(12)
        }
        .background(RustMusicPalette.background.ignoresSafeArea())
        .foregroundStyle(RustMusicPalette.phosphor)
        .preferredColorScheme(.dark)
        .tint(RustMusicPalette.phosphor)
    }
}

@MainActor
private final class RustMusicApplicationDelegate: NSObject, NSApplicationDelegate {
    private var terminationPending = false

    func applicationShouldTerminate(_ sender: NSApplication) -> NSApplication.TerminateReply {
        guard !terminationPending else { return .terminateLater }
        terminationPending = true
        RustMusicPlayerRegistry.shutdownAll {
            // AppKit expects this reply after applicationShouldTerminate exits,
            // even when every model was already idle and finished synchronously.
            DispatchQueue.main.async {
                sender.reply(toApplicationShouldTerminate: true)
            }
        }
        return .terminateLater
    }
}

@MainActor
private final class WeakRustMusicPlayerModel {
    weak var value: PlayerModel?
    init(_ value: PlayerModel) { self.value = value }
}

@MainActor
private enum RustMusicPlayerRegistry {
    private static var models: [WeakRustMusicPlayerModel] = []

    private static var liveModels: [PlayerModel] {
        models = models.filter { $0.value != nil }
        return models.compactMap(\.value)
    }

    static func register(_ model: PlayerModel) {
        let live = liveModels
        guard !live.contains(where: { $0 === model }) else { return }
        models.append(WeakRustMusicPlayerModel(model))
    }

    static func shutdownAll(completion: @escaping () -> Void) {
        let live = liveModels
        guard !live.isEmpty else {
            completion()
            return
        }
        var remaining = live.count
        for model in live {
            model.shutdownPackage {
                remaining -= 1
                if remaining == 0 { completion() }
            }
        }
    }
}

private final class CatalogViewCancellation: @unchecked Sendable {
    private let lock = NSLock()
    private var cancelled = false

    func cancel() {
        lock.lock()
        cancelled = true
        lock.unlock()
    }

    var isCancelled: Bool {
        lock.lock()
        defer { lock.unlock() }
        return cancelled
    }
}

@MainActor
final class PlayerModel: ObservableObject {
    @Published var libraryRoot: URL?
    @Published var tracks: [CatalogTrack] = [] {
        didSet { refreshVisibleTracks() }
    }
    @Published var selectedIndex: Int? {
        didSet {
            if oldValue != selectedIndex { playbackSnapshot = nil }
        }
    }
    @Published var selectedTrackIDs: Set<Int> = []
    @Published var searchText = "" {
        didSet { refreshVisibleTracks() }
    }
    @Published var sortOrder: CatalogSearch.SortOrder {
        didSet {
            UserDefaults.standard.set(sortOrder.rawValue, forKey: "RustMusic.catalogSortOrder")
            refreshVisibleTracks()
        }
    }
    @Published private(set) var visibleTracks: [CatalogTrack] = []
    @Published var status = "Choose a local library folder."
    @Published var isPlaying = false
    @Published var isPaused = false
    @Published var playbackSnapshot: PackagePlaybackSnapshot?
    @Published private(set) var audioQueueDiagnosticSummary: String?
    @Published var isTransportBusy = false
    @Published var volumePercent = 100
    @Published private(set) var outputDevices: [AudioOutputChoice] = []
    @Published private(set) var selectedOutputDeviceUID: String
    @Published private(set) var outputDeviceStatus = "Checking available outputs…"
    @Published var isScanning = false
    @Published var isStateBusy = false
    @Published var queueEntries: [QueueEntry] = []
    @Published var queueSelection: Int?
    @Published var selectedQueueIndex: Int?
    @Published var repeatMode = "off"
    @Published var shuffleSeed: String?
    @Published var playlists: [UserPlaylist] = []
    @Published var selectedPlaylist: String? {
        didSet {
            if oldValue != selectedPlaylist { selectedPlaylistEntryIDs = [] }
        }
    }
    @Published var selectedPlaylistEntryIDs: Set<Int> = []
    @Published var newPlaylistName = ""

    private var scanCancellationRequested = false
    private var scanProcess: Process?
    private var stateProcess: Process?
    private let packageQueue = DispatchQueue(label: "dev.rustmusic.package-process")
    private let catalogViewQueue = DispatchQueue(label: "dev.rustmusic.catalog-view", qos: .userInitiated)
    private var catalogViewGeneration: UInt64 = 0
    private var catalogViewCancellation: CatalogViewCancellation?
    nonisolated(unsafe) private var packageClient: PackageProcessClient?
    nonisolated(unsafe) private var packageRootPath: String?
    nonisolated(unsafe) private var packageOutputDeviceUID: String?
    private var snapshotPollScheduled = false
    private var snapshotPollGeneration: UInt64 = 0
    private var snapshotPollInFlightGeneration: UInt64?
    private var pendingStateReload = false
    private let shutdownGate = ShutdownCompletionGate()
    private let transientChildOperations = DispatchGroup()

    init() {
        sortOrder = CatalogSearch.SortOrder(
            rawValue: UserDefaults.standard.string(forKey: "RustMusic.catalogSortOrder") ?? "title"
        ) ?? .title
        selectedOutputDeviceUID = UserDefaults.standard.string(forKey: "RustMusic.outputDeviceUID") ?? ""
        RustMusicPlayerRegistry.register(self)
        refreshOutputDevices()
    }

    private var decoderURL: URL {
        Bundle.main.bundleURL.appendingPathComponent("Contents/MacOS/rust-music-cli")
    }

    var selectedTrack: CatalogTrack? {
        guard let selectedIndex else { return nil }
        return tracks.first(where: { $0.id == selectedIndex })
    }

    var selectedTracksInCatalogOrder: [CatalogTrack] {
        tracks.filter { selectedTrackIDs.contains($0.id) }
    }

    func refreshOutputDevices() {
        DispatchQueue.global(qos: .utility).async { [weak self] in
            do {
                let devices = try CoreAudioOutputDiscovery.list()
                DispatchQueue.main.async {
                    guard let self else { return }
                    self.outputDevices = devices
                    if !self.selectedOutputDeviceUID.isEmpty,
                       !devices.contains(where: { $0.uid == self.selectedOutputDeviceUID }) {
                        if self.isPlaying || self.isPaused {
                            self.outputDeviceStatus = "Selected output disappeared; stop playback to switch to System Default."
                        } else {
                            self.selectedOutputDeviceUID = ""
                            UserDefaults.standard.removeObject(forKey: "RustMusic.outputDeviceUID")
                            self.outputDeviceStatus = "Saved output is unavailable; using System Default."
                            self.recreatePackageClientForOutputChange()
                        }
                    } else {
                        self.outputDeviceStatus = devices.isEmpty
                            ? "No output devices were returned by CoreAudio."
                            : "Choose the output Rust Music should use."
                    }
                }
            } catch {
                DispatchQueue.main.async {
                    guard let self else { return }
                    if !self.selectedOutputDeviceUID.isEmpty, !self.isPlaying, !self.isPaused {
                        self.selectedOutputDeviceUID = ""
                        UserDefaults.standard.removeObject(forKey: "RustMusic.outputDeviceUID")
                        self.recreatePackageClientForOutputChange()
                        self.outputDeviceStatus = "Could not verify saved output; using System Default."
                    } else {
                        self.outputDeviceStatus = "Could not list audio outputs: \(error.localizedDescription)"
                    }
                }
            }
        }
    }

    func selectOutputDevice(_ uid: String) {
        guard !isPlaying, !isPaused, !isTransportBusy, !isStateBusy else {
            outputDeviceStatus = "Stop playback before changing its output device."
            return
        }
        guard uid.isEmpty || outputDevices.contains(where: { $0.uid == uid }) else {
            outputDeviceStatus = "That output is no longer available; refresh the list."
            return
        }
        guard uid != selectedOutputDeviceUID else { return }
        selectedOutputDeviceUID = uid
        if uid.isEmpty {
            UserDefaults.standard.removeObject(forKey: "RustMusic.outputDeviceUID")
            outputDeviceStatus = "Rust Music will use the macOS System Default output."
        } else {
            UserDefaults.standard.set(uid, forKey: "RustMusic.outputDeviceUID")
            outputDeviceStatus = "Output changed. Press Play to route through the selected device."
        }
        recreatePackageClientForOutputChange()
    }

    private func recreatePackageClientForOutputChange() {
        isTransportBusy = true
        packageQueue.async { [weak self] in
            guard let self else { return }
            self.packageClient?.shutdown()
            self.packageClient = nil
            self.packageRootPath = nil
            self.packageOutputDeviceUID = nil
            DispatchQueue.main.async { self.isTransportBusy = false }
        }
    }

    func boundSearchText() {
        let bounded = CatalogSearch.boundedQuery(searchText)
        if bounded != searchText { searchText = bounded }
        // Filtering never changes the selected playback target. A playing
        // track may be outside the current search results without switching.
    }

    private func refreshVisibleTracks() {
        catalogViewGeneration &+= 1
        let generation = catalogViewGeneration
        catalogViewCancellation?.cancel()
        let cancellation = CatalogViewCancellation()
        catalogViewCancellation = cancellation
        let catalog = tracks
        let query = CatalogSearch.boundedQuery(searchText)
        let order = sortOrder
        catalogViewQueue.asyncAfter(deadline: .now() + 0.12) { [weak self] in
            guard !cancellation.isCancelled else { return }
            let result = CatalogSearch.visibleTracks(in: catalog, matching: query, sortedBy: order)
            DispatchQueue.main.async {
                guard let self, self.catalogViewGeneration == generation else { return }
                self.visibleTracks = result
            }
        }
    }

    private func runStateCommand(
        _ arguments: [String],
        completion: @escaping (Result<Data, RustMusicFailure>) -> Void
    ) {
        guard shutdownGate.acceptsWork, libraryRoot != nil, !isStateBusy, !isTransportBusy else { return }
        let process = Process()
        process.executableURL = decoderURL
        process.arguments = ["state", "--managed-parent-stdin"] + arguments
        let input = Pipe()
        let output = Pipe()
        process.standardInput = input
        process.standardOutput = output
        process.standardError = output
        do {
            try process.run()
        } catch {
            try? input.fileHandleForWriting.close()
            completion(.failure(RustMusicFailure(
                message: "Could not start Rust Music state storage: \(error.localizedDescription)"
            )))
            return
        }
        stateProcess = process
        isStateBusy = true
        let childOperations = transientChildOperations
        childOperations.enter()
        Task { [weak self] in
            let (bytes, outputFailure, exitStatus) = await Task.detached(priority: .userInitiated) {
                defer { try? input.fileHandleForWriting.close() }
                let bytes: Data
                let outputFailure: String?
                do {
                    if let bounded = try CatalogValidation.readBounded(
                        output.fileHandleForReading,
                        maximumBytes: CatalogValidation.maximumStateOutputBytes,
                        onFailure: {
                            if process.isRunning { process.terminate() }
                        }
                    ) {
                        bytes = bounded
                        outputFailure = nil
                    } else {
                        bytes = Data()
                        outputFailure = "Rust Music state output exceeded its size limit."
                    }
                } catch {
                    bytes = Data()
                    outputFailure = "Could not read Rust Music state output: \(error.localizedDescription)"
                }
                process.waitUntilExit()
                childOperations.leave()
                return (bytes, outputFailure, process.terminationStatus)
            }.value
            guard let self, self.stateProcess === process else { return }
            self.stateProcess = nil
            self.isStateBusy = false
            if let outputFailure {
                completion(.failure(RustMusicFailure(message: outputFailure)))
            } else if exitStatus == 0 {
                completion(.success(bytes))
            } else {
                let message = String(data: bytes, encoding: .utf8)?
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                completion(.failure(RustMusicFailure(
                    message: message?.isEmpty == false ? message ?? "" : "Rust Music state update failed."
                )))
            }
        }
    }

    private func reloadPersistentState() {
        guard !isTransportBusy else {
            pendingStateReload = true
            return
        }
        pendingStateReload = false
        reloadQueue()
    }

    private func finishTransportOperation() {
        isTransportBusy = false
        if pendingStateReload { reloadPersistentState() }
    }

    private func reloadQueue() {
        guard let libraryRoot else { return }
        runStateCommand(["queue", libraryRoot.path, "list"]) { [weak self] result in
            guard let self else { return }
            switch result {
            case .success(let bytes):
                guard let queue = Self.parseQueue(bytes) else {
                    self.status = "Rust Music returned malformed queue state."
                    return
                }
                self.queueEntries = queue.entries
                self.queueSelection = queue.current
                self.selectedQueueIndex = queue.current
                self.repeatMode = queue.repeatMode
                self.shuffleSeed = queue.shuffleSeed
                if let current = queue.current,
                   let entry = queue.entries.first(where: { $0.index == current }),
                   let track = self.tracks.first(where: { $0.relativePath == entry.relativePath }) {
                    self.selectedIndex = track.id
                    self.selectedTrackIDs = [track.id]
                }
                self.reloadPlaylists()
            case .failure(let failure):
                self.status = failure.message
            }
        }
    }

    private func reloadPlaylists() {
        guard let libraryRoot else { return }
        runStateCommand(["playlist", libraryRoot.path, "list"]) { [weak self] result in
            guard let self else { return }
            switch result {
            case .success(let bytes):
                guard let playlists = Self.parsePlaylists(bytes) else {
                    self.status = "Rust Music returned malformed playlist state."
                    return
                }
                self.playlists = playlists
                if let selected = self.selectedPlaylist,
                   !playlists.contains(where: { $0.name == selected }) {
                    self.selectedPlaylist = nil
                }
                if let selected = self.playlists.first(where: { $0.name == self.selectedPlaylist }) {
                    self.selectedPlaylistEntryIDs = Set(
                        self.selectedPlaylistEntryIDs.filter { $0 >= 0 && $0 < selected.paths.count }
                    )
                }
            case .failure(let failure):
                self.status = failure.message
            }
        }
    }

    func addSelectionToQueue() {
        guard let libraryRoot else { return }
        let selected = selectedTracksInCatalogOrder
        guard !selected.isEmpty else { return }
        guard selected.count <= 64 else {
            status = "Select at most 64 tracks for one queue operation."
            return
        }
        runStateCommand(["queue", libraryRoot.path, "add-many"] + selected.map(\.relativePath)) { [weak self] result in
            self?.applyQueueResult(result)
        }
    }

    func addSelectedTrackNext() {
        guard let libraryRoot, let selectedTrack else { return }
        runStateCommand(["queue", libraryRoot.path, "play-next", selectedTrack.relativePath]) { [weak self] result in
            self?.applyQueueResult(result)
        }
    }

    func removeQueueSelection() {
        guard let libraryRoot, let selectedQueueIndex else { return }
        runStateCommand(["queue", libraryRoot.path, "remove", String(selectedQueueIndex)]) { [weak self] result in
            self?.applyQueueResult(result)
        }
    }

    func moveQueueSelection(by offset: Int) {
        guard let libraryRoot, let selectedQueueIndex else { return }
        let target = selectedQueueIndex + offset
        guard target >= 0, target < queueEntries.count else { return }
        runStateCommand(["queue", libraryRoot.path, "move", String(selectedQueueIndex), String(target)]) { [weak self] result in
            self?.applyQueueResult(result)
            self?.selectedQueueIndex = target
        }
    }

    func clearQueue() {
        guard let libraryRoot, !queueEntries.isEmpty else { return }
        runStateCommand(["queue", libraryRoot.path, "clear"]) { [weak self] result in
            self?.applyQueueResult(result)
        }
    }

    func selectQueueEntry(_ index: Int) {
        guard let libraryRoot else { return }
        selectedQueueIndex = index
        runStateCommand(["queue", libraryRoot.path, "select", String(index)]) { [weak self] result in
            self?.applyQueueResult(result, reflectQueueSelection: true)
        }
    }

    func next() {
        let continuePlayback = isPlaying || isPaused
        guard !queueEntries.isEmpty, let libraryRoot else {
            if moveSelection(forward: true), continuePlayback {
                play()
            }
            return
        }
        runStateCommand(["queue", libraryRoot.path, "next"]) { [weak self] result in
            self?.applyQueueResult(result, continuePlayback: continuePlayback)
        }
    }

    func previous() {
        let continuePlayback = isPlaying || isPaused
        guard !queueEntries.isEmpty, let libraryRoot else {
            if moveSelection(forward: false), continuePlayback {
                play()
            }
            return
        }
        runStateCommand(["queue", libraryRoot.path, "previous"]) { [weak self] result in
            self?.applyQueueResult(result, continuePlayback: continuePlayback)
        }
    }

    func cycleRepeatMode() {
        guard let libraryRoot else { return }
        let next = switch repeatMode {
        case "off": "one"
        case "one": "all"
        default: "off"
        }
        runStateCommand(["queue", libraryRoot.path, "repeat", next]) { [weak self] result in
            self?.applyQueueResult(result)
        }
    }

    func toggleShuffle() {
        guard let libraryRoot else { return }
        let action = shuffleSeed == nil
            ? ["queue", libraryRoot.path, "shuffle", String(UInt64.random(in: .min ... .max))]
            : ["queue", libraryRoot.path, "shuffle-off"]
        runStateCommand(action) { [weak self] result in
            self?.applyQueueResult(result)
        }
    }

    func createPlaylist() {
        guard let libraryRoot else { return }
        let name = newPlaylistName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty, name.utf8.count <= 128 else { return }
        runStateCommand(["playlist", libraryRoot.path, "create", name]) { [weak self] result in
            guard let self, self.applyPlaylistResult(result) else { return }
            self.selectedPlaylist = name
            self.newPlaylistName = ""
        }
    }

    func renameSelectedPlaylist() {
        guard let libraryRoot, let selectedPlaylist else { return }
        let name = newPlaylistName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty, name.utf8.count <= 128 else { return }
        runStateCommand(["playlist", libraryRoot.path, "rename", selectedPlaylist, name]) { [weak self] result in
            guard let self, self.applyPlaylistResult(result) else { return }
            self.selectedPlaylist = name
            self.newPlaylistName = ""
        }
    }

    func addSelectionToPlaylist() {
        guard let libraryRoot, let selectedPlaylist else { return }
        let selected = selectedTracksInCatalogOrder
        guard !selected.isEmpty else { return }
        guard selected.count <= 64 else {
            status = "Select at most 64 tracks for one playlist operation."
            return
        }
        runStateCommand(["playlist", libraryRoot.path, "add-many", selectedPlaylist] + selected.map(\.relativePath)) { [weak self] result in
            self?.applyPlaylistResult(result)
        }
    }

    func moveSelectedPlaylistEntries(by offset: Int) {
        guard (-1...1).contains(offset), offset != 0,
              let libraryRoot, let selectedPlaylist,
              let playlist = playlists.first(where: { $0.name == selectedPlaylist }),
              !selectedPlaylistEntryIDs.isEmpty,
              selectedPlaylistEntryIDs.allSatisfy({ $0 >= 0 && $0 < playlist.paths.count }) else { return }
        guard selectedPlaylistEntryIDs.count <= 64 else {
            status = "Select at most 64 playlist tracks for one reorder."
            return
        }
        let ordered = selectedPlaylistEntryIDs.sorted()
        let movedSelection = PlaylistSelection.indices(
            afterMoving: selectedPlaylistEntryIDs,
            in: playlist.paths.count,
            by: offset
        )
        guard !movedSelection.isEmpty else { return }
        let arguments = ordered.map { String($0) }
        runStateCommand([
            "playlist", libraryRoot.path, "move-many", selectedPlaylist, String(offset)
        ] + arguments) { [weak self] result in
            guard let self, self.applyPlaylistResult(result) else { return }
            self.selectedPlaylistEntryIDs = movedSelection
        }
    }

    func removeSelectedPlaylistEntries() {
        guard let libraryRoot, let selectedPlaylist,
              let playlist = playlists.first(where: { $0.name == selectedPlaylist }),
              !selectedPlaylistEntryIDs.isEmpty,
              selectedPlaylistEntryIDs.allSatisfy({ $0 >= 0 && $0 < playlist.paths.count }) else { return }
        guard selectedPlaylistEntryIDs.count <= 64 else {
            status = "Select at most 64 playlist tracks for one removal."
            return
        }
        let ordered = selectedPlaylistEntryIDs.sorted(by: >).map { String($0) }
        runStateCommand(["playlist", libraryRoot.path, "remove-many", selectedPlaylist] + ordered) { [weak self] result in
            guard let self, self.applyPlaylistResult(result) else { return }
            self.selectedPlaylistEntryIDs = []
        }
    }

    func deleteSelectedPlaylist() {
        guard let libraryRoot, let selectedPlaylist else { return }
        runStateCommand(["playlist", libraryRoot.path, "delete", selectedPlaylist]) { [weak self] result in
            self?.applyPlaylistResult(result)
        }
    }

    func addSelectedPlaylistToQueue() {
        guard let libraryRoot, let selectedPlaylist else { return }
        runStateCommand(["queue", libraryRoot.path, "add-playlist", selectedPlaylist]) { [weak self] result in
            self?.applyQueueResult(result)
        }
    }

    private func applyQueueResult(
        _ result: Result<Data, RustMusicFailure>,
        continuePlayback: Bool = false,
        finishedAtEnd: Bool = false,
        reflectQueueSelection: Bool = false
    ) {
        switch result {
        case .success(let bytes):
            guard let queue = Self.parseQueue(bytes) else {
                status = "Rust Music returned malformed queue state."
                return
            }
            queueEntries = queue.entries
            queueSelection = queue.current
            selectedQueueIndex = queue.current
            repeatMode = queue.repeatMode
            shuffleSeed = queue.shuffleSeed
            if continuePlayback, queue.candidateAvailable == false {
                // At Repeat Off's boundary the current entry remains selected,
                // but no candidate exists in the requested direction.
                if finishedAtEnd {
                    isPlaying = false
                    isPaused = false
                    status = "Playback finished."
                } else {
                    status = "No track in that direction. Current selection is unchanged."
                }
                return
            }
            if let current = queue.current,
               let entry = queue.entries.first(where: { $0.index == current }),
               let track = tracks.first(where: { $0.relativePath == entry.relativePath }) {
                let selectionChanged = selectedIndex != track.id
                selectedIndex = track.id
                if reflectQueueSelection || continuePlayback { selectedTrackIDs = [track.id] }
                if continuePlayback { restartQueuePlayback() }
                else if selectionChanged && (isPlaying || isPaused) { stop() }
            }
            status = "Queue saved. Selection does not start playback; press Play explicitly."
        case .failure(let failure):
            status = failure.message
        }
    }

    @discardableResult
    private func applyPlaylistResult(_ result: Result<Data, RustMusicFailure>) -> Bool {
        switch result {
        case .success(let bytes):
            guard let playlists = Self.parsePlaylists(bytes) else {
                status = "Rust Music returned malformed playlist state."
                return false
            }
            self.playlists = playlists
            if let selected = playlists.first(where: { $0.name == self.selectedPlaylist }) {
                self.selectedPlaylistEntryIDs = Set(
                    self.selectedPlaylistEntryIDs.filter { $0 >= 0 && $0 < selected.paths.count }
                )
            } else {
                self.selectedPlaylistEntryIDs = []
            }
            status = "Playlists saved. Source audio remains unchanged."
            return true
        case .failure(let failure):
            status = failure.message
            return false
        }
    }

    func chooseLibrary() {
        let picker = NSOpenPanel()
        picker.title = "Choose a Music Library Folder"
        picker.prompt = "Load Library"
        picker.canChooseFiles = false
        picker.canChooseDirectories = true
        picker.allowsMultipleSelection = false
        guard picker.runModal() == .OK, let url = picker.url else { return }
        loadLibrary(at: url)
    }

    func cancelScan() {
        guard let scanProcess, scanProcess.isRunning else { return }
        scanCancellationRequested = true
        scanProcess.terminate()
        status = "Canceling library scan… the previous catalog is preserved."
    }

    func cancelStateOperation() {
        guard let stateProcess, stateProcess.isRunning else { return }
        stateProcess.terminate()
    }

    private func loadLibrary(at root: URL) {
        guard shutdownGate.acceptsWork, !isScanning, !isStateBusy, !isTransportBusy else { return }
        playbackSnapshot = nil
        stop()
        let process = Process()
        process.executableURL = decoderURL
        process.arguments = ["catalog", "--managed-parent-stdin", root.path]
        let input = Pipe()
        let output = Pipe()
        process.standardInput = input
        process.standardOutput = output
        process.standardError = output
        do {
            try process.run()
        } catch {
            try? input.fileHandleForWriting.close()
            status = "Could not start the Rust Music catalog: \(error.localizedDescription)"
            return
        }
        scanProcess = process
        isScanning = true
        scanCancellationRequested = false
        status = "Scanning the selected folder… playback will not start."
        let childOperations = transientChildOperations
        childOperations.enter()
        Task { [weak self] in
            let result = await Task.detached(priority: .userInitiated) {
                defer { try? input.fileHandleForWriting.close() }
                let bytes: Data
                let outputFailure: String?
                do {
                    if let bounded = try CatalogValidation.readBounded(
                        output.fileHandleForReading,
                        maximumBytes: CatalogValidation.maximumOutputBytes,
                        onFailure: {
                            if process.isRunning { process.terminate() }
                        }
                    ) {
                        bytes = bounded
                        outputFailure = nil
                    } else {
                        bytes = Data()
                        outputFailure = "Rust Music catalog exceeded its output limit."
                    }
                } catch {
                    bytes = Data()
                    outputFailure = "Could not read Rust Music catalog output: \(error.localizedDescription)"
                }
                process.waitUntilExit()
                let result: Result<[CatalogTrack], RustMusicFailure>
                if let outputFailure {
                    result = .failure(RustMusicFailure(message: outputFailure))
                } else if process.terminationStatus == 0 {
                    result = Self.parseCatalog(bytes)
                } else {
                    let message = String(data: bytes, encoding: .utf8)?
                        .trimmingCharacters(in: .whitespacesAndNewlines)
                    result = .failure(RustMusicFailure(
                        message: message?.isEmpty == false ? message ?? "" : "The catalog scan failed."
                    ))
                }
                childOperations.leave()
                return result
            }.value
            guard let self, self.scanProcess === process else { return }
            self.scanProcess = nil
            self.isScanning = false
            let wasCancelled = self.scanCancellationRequested
            self.scanCancellationRequested = false
            switch result {
            case .success(let tracks):
                self.libraryRoot = root
                self.tracks = tracks
                self.selectedIndex = tracks.first?.id
                self.selectedTrackIDs = tracks.first.map { Set([$0.id]) } ?? []
                self.status = "Loaded \(tracks.count) tracks. Playback is stopped."
                self.reloadPersistentState()
            case .failure(let failure):
                self.status = wasCancelled
                    ? "Library scan canceled; the previous catalog is preserved."
                    : failure.message
            }
        }
    }

    func play() {
        guard let selectedTrack, !isScanning, !isStateBusy, !isTransportBusy,
              let index = UInt16(exactly: selectedTrack.id) else { return }
        audioQueueDiagnosticSummary = nil
        let volume = UInt8(volumePercent)
        runPackageCommands([.select(index), .setVolumePercent(volume), .play], label: "Starting playback…") { [weak self] response in
            self?.applyPlaybackResponse(response, fallback: "Playback started.")
        }
    }

    private func restartQueuePlayback() {
        guard let selectedTrack,
              let index = UInt16(exactly: selectedTrack.id) else { return }
        audioQueueDiagnosticSummary = nil
        let volume = UInt8(volumePercent)
        runPackageCommands([.stop, .select(index), .setVolumePercent(volume), .play], label: "Advancing queue…") { [weak self] response in
            self?.applyPlaybackResponse(response, fallback: "Queue advanced.")
        }
    }

    func stop() {
        guard !isStateBusy, !isTransportBusy, libraryRoot != nil, isPlaying || isPaused else { return }
        runPackageCommands([.stop], label: "Stopping playback…") { [weak self] response in
            self?.applyPlaybackResponse(response, fallback: "Playback stopped.")
        }
    }

    func togglePause() {
        guard !isStateBusy, !isTransportBusy, libraryRoot != nil else { return }
        let request: PackageRequest = isPaused ? .resumePlayback : .pause
        runPackageCommands([request], label: isPaused ? "Resuming playback…" : "Pausing playback…") { [weak self] response in
            self?.applyPlaybackResponse(response, fallback: "Playback state updated.")
        }
    }

    func commitVolume(_ percent: Int) {
        guard !isStateBusy, !isTransportBusy, libraryRoot != nil,
              let bounded = UInt8(exactly: min(100, max(0, percent))) else { return }
        volumePercent = Int(bounded)
        guard isPlaying || isPaused else {
            status = "Volume set to \(bounded)%; it will apply when playback starts."
            return
        }
        runPackageCommands([.setVolumePercent(bounded)], label: "Setting volume to \(bounded)%…") { [weak self] response in
            self?.applyPlaybackResponse(response, fallback: "Volume set to \(bounded)%.")
        }
    }

    func seek(toPerMille perMille: Int) {
        guard isPlaying, !isTransportBusy, !isStateBusy,
              let totalFrames = playbackSnapshot?.totalFrames,
              let frame = PackagePlaybackSnapshot.seekFrame(
                totalFrames: totalFrames,
                perMille: perMille
              ) else { return }
        // Seeking drains and closes the current AudioQueue, then leaves the
        // decoder Ready. Reissue Play explicitly so scrubbing during playback
        // resumes from the requested position with a fresh output queue.
        let volume = UInt8(volumePercent)
        runPackageCommands(
            [.seekFrame(frame), .setVolumePercent(volume), .play],
            label: "Seeking…"
        ) { [weak self] response in
            self?.applyPlaybackResponse(response, fallback: "Playback resumed from the selected position.")
        }
    }

    private func runPackageCommands(
        _ requests: [PackageRequest],
        label: String,
        completion: @escaping (PackageResponse) -> Void
    ) {
        guard shutdownGate.acceptsWork, let root = libraryRoot, !isTransportBusy, !isStateBusy else { return }
        invalidateSnapshotPolls()
        isTransportBusy = true
        status = label
        let executable = decoderURL
        let outputDeviceUID = selectedOutputDeviceUID.isEmpty ? nil : selectedOutputDeviceUID
        packageQueue.async { [weak self] in
            guard let self else { return }
            do {
                let rootPath = root.standardizedFileURL.path
                if self.packageRootPath != rootPath || self.packageOutputDeviceUID != outputDeviceUID {
                    self.packageClient?.shutdown()
                    self.packageClient = try PackageProcessClient(
                        executable: executable,
                        root: root,
                        outputDeviceUID: outputDeviceUID
                    )
                    self.packageRootPath = rootPath
                    self.packageOutputDeviceUID = outputDeviceUID
                }
                guard let client = self.packageClient else {
                    throw RustMusicFailure(message: "Rust Music playback process is unavailable.")
                }
                var response: PackageResponse = .ack
                for request in requests {
                    response = try client.request(request)
                    if case .failure = response { break }
                }
                DispatchQueue.main.async {
                    self.finishTransportOperation()
                    completion(response)
                }
            } catch {
                self.packageClient?.shutdown(force: true)
                self.packageClient = nil
                self.packageRootPath = nil
                self.packageOutputDeviceUID = nil
                DispatchQueue.main.async {
                    self.finishTransportOperation()
                    self.isPlaying = false
                    self.isPaused = false
                    self.playbackSnapshot = nil
                    self.status = "Rust Music playback failed: \(error.localizedDescription)"
                }
            }
        }
    }

    private func invalidateSnapshotPolls() {
        snapshotPollGeneration &+= 1
        snapshotPollScheduled = false
    }

    private func applyPlaybackResponse(
        _ response: PackageResponse,
        fallback: String,
        retryStateAfterError: Bool = true
    ) {
        switch response {
        case .view(let snapshot, let diagnostic):
            if let diagnostic { audioQueueDiagnosticSummary = diagnostic }
            if PlaybackUIState.shouldClearProgress(snapshot: snapshot, selectedIndex: selectedIndex) {
                playbackSnapshot = nil
            } else {
                playbackSnapshot = snapshot
            }
            let state = snapshot.state
            let flags = PlaybackUIState.flags(for: state)
            isPlaying = flags.playing
            isPaused = flags.paused
            if state != 2 { snapshotPollScheduled = false }
            switch state {
            case 2 where !snapshot.audioClaimed:
                status = "Playback state is active, but Rust Music has no output claim. Audible output is unconfirmed."
            case 2 where snapshot.framesDelivered == 0:
                status = "Output is claimed; waiting for PCM frames. This does not confirm audible output."
            case 2:
                status = "AudioQueue accepted \(snapshot.framesDelivered) PCM frames. This does not confirm audible output."
            case 6: status = "Paused. Audio focus is released until Resume is requested."
            case 3: status = "Playback stopped."
            case 4: status = "Playback finished."
            case 5: status = "Playback failed. Check the macOS sound output and retry."
            default: status = fallback
            }
            if let diagnostic { status += " | \(diagnostic)" }
            // A failed native output must release the UI's transport lockout
            // so the user can explicitly retry after restoring the device.
            if state == 5 {
                isPlaying = false
                isPaused = false
                snapshotPollScheduled = false
            }
            if isPlaying { scheduleSnapshotPoll() }
        case .failure(let message, let diagnostic):
            playbackSnapshot = nil
            if let diagnostic { audioQueueDiagnosticSummary = diagnostic }
            let shouldReconcileState = (isPlaying || isPaused) &&
                !message.lowercased().hasPrefix("audio output:")
            status = diagnostic.map { "\(message) | \($0)" } ?? message
            isPlaying = false
            isPaused = false
            snapshotPollScheduled = false
            if retryStateAfterError && shouldReconcileState {
                runPackageCommands([.snapshot(0)], label: "") { [weak self] response in
                    self?.applyPlaybackResponse(response, fallback: "Playback state checked.", retryStateAfterError: false)
                }
            }
        case .hello, .ack:
            status = fallback
        }
    }

    private func scheduleSnapshotPoll() {
        guard !snapshotPollScheduled, snapshotPollInFlightGeneration == nil else { return }
        snapshotPollScheduled = true
        let generation = snapshotPollGeneration
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { [weak self] in
            guard let self else { return }
            guard generation == self.snapshotPollGeneration else { return }
            self.snapshotPollScheduled = false
            guard self.isPlaying, !self.isTransportBusy, !self.isStateBusy else { return }
            self.requestSnapshotPoll(generation: generation)
        }
    }

    private func requestSnapshotPoll(generation: UInt64) {
        guard generation == snapshotPollGeneration,
              snapshotPollInFlightGeneration == nil,
              shutdownGate.acceptsWork,
              isPlaying,
              !isTransportBusy,
              !isStateBusy,
              let root = libraryRoot else { return }
        snapshotPollInFlightGeneration = generation
        let rootPath = root.standardizedFileURL.path

        // Polls share the package-process serial queue but do not set the
        // interactive transport lock or replace the visible status message.
        packageQueue.async { [weak self] in
            guard let self else { return }
            do {
                guard self.packageRootPath == rootPath,
                      let client = self.packageClient else {
                    throw RustMusicFailure(message: "Rust Music playback process is unavailable.")
                }
                let response = try client.request(.snapshot(0))
                DispatchQueue.main.async {
                    self.finishSnapshotPoll(response, generation: generation)
                }
            } catch {
                let message = error.localizedDescription
                DispatchQueue.main.async {
                    self.finishSnapshotPollFailure(message, generation: generation)
                }
            }
        }
    }

    private func finishSnapshotPoll(_ response: PackageResponse, generation: UInt64) {
        guard snapshotPollInFlightGeneration == generation else { return }
        snapshotPollInFlightGeneration = nil
        guard generation == snapshotPollGeneration else {
            // A user command may have completed before this poll's main-queue
            // handler. If its response tried to schedule while this slot was
            // still occupied, restart polling now that the stale slot is clear.
            if isPlaying, !isPaused, !isTransportBusy, !isStateBusy {
                scheduleSnapshotPoll()
            }
            return
        }

        switch response {
        case .view(let snapshot, let diagnostic):
            if let diagnostic { audioQueueDiagnosticSummary = diagnostic }
            if !PlaybackUIState.shouldClearProgress(snapshot: snapshot, selectedIndex: selectedIndex) {
                playbackSnapshot = snapshot
            }
            let flags = PlaybackUIState.flags(for: snapshot.state)
            isPlaying = flags.playing
            isPaused = flags.paused
            switch snapshot.state {
            case 2:
                // Keep the status stable while only the progress snapshot changes.
                scheduleSnapshotPoll()
            case 4:
                status = diagnostic.map { "Playback finished. | \($0)" } ?? "Playback finished."
                advanceQueueAfterEnd()
            case 3:
                status = "Playback stopped."
                snapshotPollScheduled = false
            case 5:
                status = "Playback failed. Check the macOS sound output and retry."
                snapshotPollScheduled = false
            case 6:
                status = "Paused. Audio focus is released until Resume is requested."
                snapshotPollScheduled = false
            default:
                status = "Playback state updated."
                snapshotPollScheduled = false
            }
        case .failure(let message, let diagnostic):
            if let diagnostic { audioQueueDiagnosticSummary = diagnostic }
            playbackSnapshot = nil
            isPlaying = false
            isPaused = false
            snapshotPollScheduled = false
            status = diagnostic.map { "\(message) | \($0)" } ?? message
        case .hello, .ack:
            snapshotPollScheduled = false
            status = "Rust Music returned an invalid playback snapshot."
            isPlaying = false
            isPaused = false
        }
    }

    private func finishSnapshotPollFailure(_ message: String, generation: UInt64) {
        guard snapshotPollInFlightGeneration == generation else { return }
        snapshotPollInFlightGeneration = nil
        guard generation == snapshotPollGeneration else {
            if isPlaying, !isPaused, !isTransportBusy, !isStateBusy {
                scheduleSnapshotPoll()
            }
            return
        }
        snapshotPollScheduled = false
        isPlaying = false
        isPaused = false
        playbackSnapshot = nil
        status = "Rust Music playback failed: \(message)"
        // Cleanup is serialized behind this poll and ahead of any action the
        // user can issue after the failure state becomes visible. Stale poll
        // errors never tear down a client used by a newer transport command.
        packageQueue.async { [weak self] in
            self?.packageClient?.shutdown(force: true)
            self?.packageClient = nil
            self?.packageRootPath = nil
            self?.packageOutputDeviceUID = nil
        }
    }

    func shutdownPackage(completion: (() -> Void)? = nil) {
        shutdownGate.run(completion: completion) { [self] finish in
            invalidateSnapshotPolls()
            cancelScan(); cancelStateOperation()
            isTransportBusy = true
            let transientProcesses = [scanProcess, stateProcess].compactMap { $0 }
            let childOperations = transientChildOperations
            // Retain the shared model through serialized app-termination cleanup.
            packageQueue.async { [self] in
                self.packageClient?.shutdown()
                self.packageClient = nil
                self.packageRootPath = nil
                self.packageOutputDeviceUID = nil
                for process in transientProcesses {
                    ProcessTermination.ensureStopped(process, gracefulTimeout: 2, terminationTimeout: 1)
                }
                // Scan/state reader tasks own their waitUntilExit calls. Do not
                // let AppKit terminate until their output readers also finish.
                childOperations.wait()
                DispatchQueue.main.async {
                    self.isTransportBusy = false
                    self.isPlaying = false
                    self.isPaused = false
                    self.playbackSnapshot = nil
                    finish()
                }
            }
        }
    }

    func selectTracksManually(_ ids: Set<Int>) {
        selectedTrackIDs = ids
        guard let selected = visibleTracks.last(where: { ids.contains($0.id) })?.id else { return }
        let id: Int? = selected
        guard selectedIndex != id else { return }
        selectedIndex = id
        if isPlaying || isPaused { stop() }
    }

    private func advanceQueueAfterEnd() {
        guard let libraryRoot, !queueEntries.isEmpty else {
            isPlaying = false
            isPaused = false
            status = "Playback finished."
            return
        }
        runStateCommand(["queue", libraryRoot.path, "next"]) { [weak self] result in
            self?.applyQueueResult(result, continuePlayback: true, finishedAtEnd: true)
        }
    }

    @discardableResult
    private func moveSelection(forward: Bool) -> Bool {
        guard !tracks.isEmpty else { return false }
        let current = selectedIndex.flatMap { id in tracks.firstIndex(where: { $0.id == id }) }
        let next: Int
        if forward {
            next = min((current ?? -1) + 1, tracks.count - 1)
        } else {
            next = max((current ?? tracks.count) - 1, 0)
        }
        let changed = selectedIndex != tracks[next].id
        selectedIndex = tracks[next].id
        selectedTrackIDs = [tracks[next].id]
        status = "Selected \(tracks[next].title). Press Play to start output."
        return changed
    }

    nonisolated private static func parseCatalog(_ data: Data) -> Result<[CatalogTrack], RustMusicFailure> {
        let maximumTrackCount = 20_000
        guard data.count <= CatalogValidation.maximumOutputBytes else {
            return .failure(RustMusicFailure(message: "Rust Music catalog exceeded its output limit."))
        }
        guard let output = String(data: data, encoding: .utf8) else {
            return .failure(RustMusicFailure(message: "Rust Music returned invalid catalog text."))
        }
        var tracks: [CatalogTrack] = []
        let estimatedRows = min(maximumTrackCount, data.count / 96)
        tracks.reserveCapacity(estimatedRows)
        var reportedCount: Int?
        for line in output.split(whereSeparator: \.isNewline) {
            let fields = line.split(separator: "\t", omittingEmptySubsequences: false)
            if fields.first == "summary" {
                guard fields.count == 4,
                      let count = Int(fields[1]), (0...maximumTrackCount).contains(count),
                      Int(fields[2]) != nil, Int(fields[3]) != nil,
                      reportedCount == nil else {
                    return .failure(RustMusicFailure(message: "Rust Music returned a malformed catalog summary."))
                }
                reportedCount = count
                continue
            }
            guard tracks.count < maximumTrackCount,
                  fields.first == "track", fields.count == 7,
                  let index = Int(fields[1]),
                  CatalogValidation.acceptsIndex(index, after: tracks.count, maximum: maximumTrackCount),
                  let title = decodeHex(fields[3]),
                  let artist = decodeHex(fields[4]),
                  let album = decodeHex(fields[5]),
                  let relativePath = decodeHex(fields[6]),
                  !relativePath.isEmpty else {
                return .failure(RustMusicFailure(message: "Rust Music returned a malformed catalog row."))
            }
            tracks.append(CatalogTrack(
                id: index,
                container: String(fields[2]),
                title: title,
                artist: artist,
                album: album,
                relativePath: relativePath
            ))
        }
        guard reportedCount == tracks.count else {
            return .failure(RustMusicFailure(message: "Rust Music catalog count did not match its rows."))
        }
        return .success(tracks)
    }

    nonisolated private static func decodeHex(_ value: Substring) -> String? {
        guard value.count.isMultiple(of: 2) else { return nil }
        var bytes: [UInt8] = []
        bytes.reserveCapacity(value.count / 2)
        var index = value.startIndex
        while index < value.endIndex {
            let next = value.index(index, offsetBy: 2)
            guard let byte = UInt8(value[index..<next], radix: 16) else { return nil }
            bytes.append(byte)
            index = next
        }
        return String(data: Data(bytes), encoding: .utf8)
    }

    nonisolated private static func parseQueue(_ data: Data) -> ParsedQueue? {
        guard let text = String(data: data, encoding: .utf8) else { return nil }
        var repeatMode: String?
        var shuffleSeed: String?
        var shuffleSeen = false
        var current: Int?
        var currentSeen = false
        var candidateAvailable: Bool?
        var entries: [QueueEntry] = []
        var selectedEntry: Int?
        for line in text.split(whereSeparator: \.isNewline) {
            let fields = line.split(separator: "\t", omittingEmptySubsequences: false)
            if fields.first == "meta" {
                guard fields.count == 3 else { return nil }
                switch fields[1] {
                case "repeat":
                    guard repeatMode == nil, ["off", "one", "all"].contains(String(fields[2])) else { return nil }
                    repeatMode = String(fields[2])
                case "shuffle":
                    guard !shuffleSeen else { return nil }
                    shuffleSeen = true
                    if fields[2] != "-" {
                        guard UInt64(fields[2]) != nil else { return nil }
                        shuffleSeed = String(fields[2])
                    }
                case "current":
                    guard !currentSeen else { return nil }
                    currentSeen = true
                    if fields[2] != "-" {
                        guard let value = Int(fields[2]), value >= 0 else { return nil }
                        current = value
                    }
                case "candidate-available":
                    guard candidateAvailable == nil else { return nil }
                    switch fields[2] {
                    case "yes": candidateAvailable = true
                    case "no": candidateAvailable = false
                    default: return nil
                    }
                default: return nil
                }
                continue
            }
            guard fields.first == "entry", fields.count == 4,
                  let index = Int(fields[1]), index == entries.count,
                  fields[2] == "0" || fields[2] == "1",
                  let path = decodeHex(fields[3]), !path.isEmpty else { return nil }
            let isSelected = fields[2] == "1"
            if isSelected {
                guard selectedEntry == nil else { return nil }
                selectedEntry = index
            }
            entries.append(QueueEntry(index: index, isSelected: isSelected, relativePath: path))
        }
        guard let repeatMode, shuffleSeen, currentSeen,
              current.map({ $0 < entries.count }) ?? true,
              current == selectedEntry else { return nil }
        return ParsedQueue(
            entries: entries,
            repeatMode: repeatMode,
            shuffleSeed: shuffleSeed,
            current: current,
            candidateAvailable: candidateAvailable
        )
    }

    nonisolated private static func parsePlaylists(_ data: Data) -> [UserPlaylist]? {
        guard let text = String(data: data, encoding: .utf8) else { return nil }
        var playlists: [UserPlaylist] = []
        var currentName: String?
        var expectedEntries: Int?
        var paths: [String] = []
        func finishCurrent() -> Bool {
            guard let name = currentName, let expectedCount = expectedEntries,
                  expectedCount == paths.count else { return false }
            guard !playlists.contains(where: { $0.name == name }) else { return false }
            playlists.append(UserPlaylist(name: name, paths: paths))
            currentName = nil
            expectedEntries = nil
            paths = []
            return true
        }
        for line in text.split(whereSeparator: \.isNewline) {
            let fields = line.split(separator: "\t", omittingEmptySubsequences: false)
            if fields.first == "playlist" {
                if currentName != nil && !finishCurrent() { return nil }
                guard fields.count == 3,
                      let name = decodeHex(fields[1]), !name.isEmpty,
                      name.utf8.count <= 128,
                      let count = Int(fields[2]), count >= 0, count <= 10_000,
                      playlists.count < 128 else { return nil }
                currentName = name
                expectedEntries = count
                paths = []
                continue
            }
            guard fields.first == "entry", fields.count == 4,
                  let name = decodeHex(fields[1]), name == currentName,
                  let index = Int(fields[2]), index == paths.count,
                  let path = decodeHex(fields[3]), !path.isEmpty else { return nil }
            paths.append(path)
            if paths.count > 10_000 { return nil }
        }
        if currentName != nil && !finishCurrent() { return nil }
        return playlists
    }
}

private struct PlayerView: View {
    @ObservedObject private var model: PlayerModel
    private let kind: PlayerWindowKind
    @State private var progressScrubPerMille: Double?

    init(model: PlayerModel, kind: PlayerWindowKind) {
        self.model = model
        self.kind = kind
    }

    var body: some View {
        PipBoyChassis {
            VStack(spacing: 0) {
                ScrollView {
                    VStack(alignment: .leading, spacing: 12) {
                        header
                        Group {
                            switch kind {
                            case .player:
                                nowPlayingPanel
                            case .playlist:
                                VStack(alignment: .leading, spacing: 12) {
                                    queuePanel
                                    playlistPanel
                                }
                            case .library:
                                libraryPanel
                            }
                        }
                    }
                    .padding(10)
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .scrollIndicators(.visible)

                if kind == .player { transportBar }
            }
        }
        .background(RustMusicPalette.background.ignoresSafeArea())
        .foregroundStyle(RustMusicPalette.phosphor)
        .preferredColorScheme(.dark)
        .tint(RustMusicPalette.phosphor)
        .onChange(of: model.searchText) { _ in model.boundSearchText() }
    }

    private var header: some View {
        HStack(alignment: .center, spacing: 12) {
            VStack(alignment: .leading, spacing: 2) {
                    Text("RUST MUSIC / AUDIO")
                    .font(.system(size: 19, weight: .bold, design: .monospaced))
                    .tracking(1.1)
                    .foregroundStyle(RustMusicPalette.phosphor)
                Text("LOCAL LOSSLESS LIBRARY")
                    .font(.system(size: 10, weight: .medium, design: .monospaced))
                    .tracking(1.4)
                    .foregroundStyle(RustMusicPalette.muted)
            }
            Spacer(minLength: 12)
            WindowSwitcher()
            Button(model.isScanning ? "CANCEL SCAN" : "LOAD LIBRARY") {
                if model.isScanning { model.cancelScan() } else { model.chooseLibrary() }
            }
            .buttonStyle(RustMusicButtonStyle(prominent: true))
            .disabled(model.isScanning ? false : (model.isStateBusy || model.isTransportBusy))
            .keyboardShortcut("o", modifiers: .command)
        }
        .padding(.bottom, 8)
        .overlay(alignment: .bottom) {
            Rectangle().fill(RustMusicPalette.line).frame(height: 1)
        }
    }

    private var nowPlayingPanel: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 9) {
                HStack {
                    Text(model.isPlaying ? "PLAYING" : model.isPaused ? "PAUSED" : "READY")
                        .font(.system(size: 10, weight: .bold, design: .monospaced))
                        .tracking(1.3)
                        .foregroundStyle(model.isPlaying ? RustMusicPalette.phosphor : RustMusicPalette.muted)
                    Spacer()
                    Text(model.selectedTrack.map { String(format: "TRACK %03d", $0.id + 1) } ?? "NO TRACK")
                        .font(.system(size: 10, weight: .medium, design: .monospaced))
                        .foregroundStyle(RustMusicPalette.muted)
                }
                Text(model.selectedTrack?.title ?? "Choose a track from your library")
                    .font(.system(size: 23, weight: .semibold, design: .monospaced))
                    .lineLimit(2)
                    .minimumScaleFactor(0.72)
                    .foregroundStyle(RustMusicPalette.phosphor)
                Text(nowPlayingDetail)
                    .font(.system(size: 12, weight: .regular, design: .monospaced))
                    .lineLimit(1)
                    .foregroundStyle(RustMusicPalette.muted)
                HStack {
                    Text(model.playbackSnapshot?.elapsedText ?? "--:--")
                    Spacer()
                    Text(model.playbackSnapshot?.durationText ?? "--:--")
                }
                .font(.system(size: 11, weight: .medium, design: .monospaced))
                .foregroundStyle(RustMusicPalette.muted)
                HStack(spacing: 8) {
                    Picker("OUTPUT", selection: Binding(
                        get: { model.selectedOutputDeviceUID },
                        set: { model.selectOutputDevice($0) }
                    )) {
                        Text("SYSTEM DEFAULT").tag("")
                        if !model.selectedOutputDeviceUID.isEmpty,
                           !model.outputDevices.contains(where: { $0.uid == model.selectedOutputDeviceUID }) {
                            Text("SAVED OUTPUT UNAVAILABLE").tag(model.selectedOutputDeviceUID)
                        }
                        ForEach(model.outputDevices) { device in
                            Text(device.isSystemDefault ? "\(device.name) · SYSTEM DEFAULT" : device.name)
                                .tag(device.uid)
                        }
                    }
                    .pickerStyle(.menu)
                    .accessibilityLabel("Audio output device")
                    .accessibilityIdentifier("audio-output-device-picker")
                    .disabled(model.isPlaying || model.isPaused || model.isTransportBusy || model.isStateBusy)
                    Button {
                        model.refreshOutputDevices()
                    } label: {
                        Image(systemName: "arrow.clockwise")
                    }
                    .buttonStyle(.bordered)
                    .help("Refresh available audio outputs")
                    .accessibilityLabel("Refresh audio outputs")
                    .disabled(model.isTransportBusy || model.isStateBusy)
                    Text(model.outputDeviceStatus.uppercased())
                        .font(.system(size: 9, weight: .regular, design: .monospaced))
                        .foregroundStyle(RustMusicPalette.muted)
                        .lineLimit(1)
                        .accessibilityIdentifier("audio-output-device-status")
                }
                if let snapshot = model.playbackSnapshot,
                   let progress = snapshot.progressPerMille {
                    Slider(
                        value: Binding(
                            get: { progressScrubPerMille ?? Double(progress) },
                            set: { progressScrubPerMille = min(1000, max(0, $0)) }
                        ),
                        in: 0...1000,
                        onEditingChanged: { editing in
                            if editing {
                                progressScrubPerMille = Double(progress)
                            } else {
                                let target = Int((progressScrubPerMille ?? Double(progress)).rounded())
                                progressScrubPerMille = nil
                                model.seek(toPerMille: target)
                            }
                        }
                    )
                    .disabled(
                        !model.isPlaying || model.isTransportBusy || model.isStateBusy
                            || snapshot.totalFrames == nil || model.selectedTrack?.container != "flac"
                    )
                    .accessibilityLabel("Playback position")
                    .accessibilityValue(snapshot.progressAccessibilityValue)
                    .accessibilityHint("Drag left to rewind or right to seek forward. Playback resumes when released.")
                    .accessibilityIdentifier("playback-progress")
                } else if model.playbackSnapshot != nil {
                    ProgressView()
                        .accessibilityLabel("Playback progress")
                        .accessibilityValue(model.playbackSnapshot?.progressAccessibilityValue ?? "Unknown duration")
                        .accessibilityIdentifier("playback-progress")
                } else {
                    ProgressView(value: 0, total: 1)
                        .accessibilityLabel("Playback progress")
                        .accessibilityValue("No playback snapshot")
                        .accessibilityIdentifier("playback-progress")
                }
                Text(model.audioQueueDiagnosticSummary.map { "\($0)" }
                     ?? model.playbackSnapshot?.outputSummary
                     ?? "NO OUTPUT SNAPSHOT · AUDIO NOT CONFIRMED")
                    .font(.system(size: 9, weight: .regular, design: .monospaced))
                    .lineLimit(4)
                    .foregroundStyle(RustMusicPalette.muted)
                    .accessibilityIdentifier("playback-output-diagnostics")
                Spacer(minLength: 0)
                Text(model.status.uppercased())
                    .font(.system(size: 10, weight: .regular, design: .monospaced))
                    .lineLimit(2)
                    .foregroundStyle(RustMusicPalette.muted)
                    .accessibilityIdentifier("playback-status")
            }
                    .frame(maxWidth: .infinity, minHeight: 92, alignment: .leading)
            .padding(12)
            .background(RustMusicPalette.display, in: RoundedRectangle(cornerRadius: 5))
        } label: {
            Text("NOW PLAYING")
        }
        .groupBoxStyle(RustMusicGroupBoxStyle())
        .accessibilityIdentifier("now-playing-panel")
    }

    private var nowPlayingDetail: String {
        guard let track = model.selectedTrack else {
            return "Playback starts only after you press Play."
        }
        let creator = [track.artist, track.album].filter { !$0.isEmpty }.joined(separator: " · ")
        return creator.isEmpty ? "\(track.container.uppercased()) · \(track.relativePath)" : creator
    }

    private var queuePanel: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 7) {
                List {
                    ForEach(model.queueEntries) { entry in
                        Button {
                            model.selectQueueEntry(entry.index)
                        } label: {
                            let track = model.tracks.first(where: { $0.relativePath == entry.relativePath })
                            HStack(spacing: 6) {
                                Text(entry.isSelected ? "▶" : " ")
                                    .foregroundStyle(RustMusicPalette.phosphor)
                                Text(track?.title ?? URL(fileURLWithPath: entry.relativePath).lastPathComponent)
                                    .lineLimit(1)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                        }
                        .buttonStyle(.plain)
                        .listRowBackground(RustMusicPalette.display)
                    }
                }
                .listStyle(.plain)
                .scrollContentBackground(.hidden)
                .background(RustMusicPalette.display)
                .frame(height: kind == .playlist ? 220 : 86)
                .disabled(model.isTransportBusy || model.isStateBusy)

                HStack(spacing: 6) {
                    Button("MOVE UP") { model.moveQueueSelection(by: -1) }
                        .disabled(model.selectedQueueIndex == nil || model.selectedQueueIndex == 0 || model.isStateBusy || model.isTransportBusy)
                    Button("MOVE DOWN") { model.moveQueueSelection(by: 1) }
                        .disabled(model.selectedQueueIndex == nil || (model.selectedQueueIndex ?? 0) >= model.queueEntries.count - 1 || model.isStateBusy || model.isTransportBusy)
                    Button("REMOVE") { model.removeQueueSelection() }
                        .disabled(model.selectedQueueIndex == nil || model.isStateBusy || model.isTransportBusy)
                    Button("CLEAR") { model.clearQueue() }
                        .disabled(model.queueEntries.isEmpty || model.isStateBusy || model.isTransportBusy)
                }
                HStack(spacing: 6) {
                    Button("REPEAT: \(model.repeatMode.uppercased())") { model.cycleRepeatMode() }
                        .disabled(model.isStateBusy || model.isTransportBusy)
                    Button(model.shuffleSeed == nil ? "SHUFFLE: OFF" : "SHUFFLE: ON") { model.toggleShuffle() }
                        .disabled(model.isStateBusy || model.isTransportBusy)
                }
            }
        } label: {
            Text("PLAY QUEUE · \(model.queueEntries.count)")
        }
        .groupBoxStyle(RustMusicGroupBoxStyle())
    }

    private var libraryPanel: some View {
        let visibleTracks = model.visibleTracks
        return GroupBox {
            VStack(alignment: .leading, spacing: 8) {
                HStack(spacing: 8) {
                    TextField("SEARCH TITLE, ARTIST, ALBUM OR PATH", text: $model.searchText)
                        .modifier(RustMusicTextFieldStyle())
                        .disabled(model.tracks.isEmpty || model.isScanning || model.isTransportBusy)
                        .accessibilityLabel("Search library by title, artist, album, or path")
                    Picker("SORT", selection: $model.sortOrder) {
                        ForEach(CatalogSearch.SortOrder.allCases) { order in
                            Text(order.label).tag(order)
                        }
                    }
                    .pickerStyle(.menu)
                    .frame(width: 108)
                    .accessibilityLabel("Sort library tracks")
                    .accessibilityIdentifier("library-sort-order")
                    Text("\(visibleTracks.count) TRACKS")
                        .font(.system(size: 10, weight: .medium, design: .monospaced))
                        .foregroundStyle(RustMusicPalette.muted)
                        .fixedSize()
                }

                List(selection: Binding<Set<Int>>(
                    get: { model.selectedTrackIDs },
                    set: { model.selectTracksManually($0) }
                )) {
                    ForEach(visibleTracks) { track in
                        VStack(alignment: .leading, spacing: 2) {
                            Text(track.title)
                                .font(.system(size: 12, weight: .semibold, design: .monospaced))
                                .lineLimit(1)
                            Text([track.artist, track.album, track.container.uppercased()]
                                .filter { !$0.isEmpty }.joined(separator: " · "))
                                .font(.system(size: 10, weight: .regular, design: .monospaced))
                                .foregroundStyle(RustMusicPalette.muted)
                                .lineLimit(1)
                        }
                        .padding(.vertical, 2)
                        .tag(track.id)
                        .listRowBackground(RustMusicPalette.display)
                    }
                }
                .listStyle(.plain)
                .scrollContentBackground(.hidden)
                .background(RustMusicPalette.display)
                .frame(minHeight: 170, maxHeight: 230)
                .disabled(model.isTransportBusy || model.isStateBusy)
                .accessibilityLabel("Library tracks")
                .accessibilityHint("Use Command-click or Shift-click to select multiple tracks, then Tab to the actions.")
                .accessibilityIdentifier("library-track-list")

                HStack(spacing: 8) {
                    Button("PLAY SELECTED") { model.play() }
                        .buttonStyle(RustMusicButtonStyle(prominent: true))
                        .disabled(model.selectedTrack == nil || model.isPlaying || model.isPaused || model.isScanning || model.isStateBusy || model.isTransportBusy)
                    Button("ENQUEUE SELECTED · \(model.selectedTrackIDs.count)") {
                        model.addSelectionToQueue()
                    }
                    .disabled(model.selectedTrackIDs.isEmpty || model.isStateBusy || model.isTransportBusy)
                    Button("PLAY NEXT") { model.addSelectedTrackNext() }
                        .disabled(model.selectedTrack == nil || model.isStateBusy || model.isTransportBusy)
                }
            }
        } label: {
            Text("LIBRARY")
        }
        .groupBoxStyle(RustMusicGroupBoxStyle())
    }

    private var playlistPanel: some View {
        GroupBox {
            VStack(alignment: .leading, spacing: 7) {
                TextField("NEW PLAYLIST NAME", text: $model.newPlaylistName)
                    .modifier(RustMusicTextFieldStyle())

                List {
                    ForEach(model.playlists) { playlist in
                        Button {
                            model.selectedPlaylist = playlist.name
                        } label: {
                            HStack {
                                Text(model.selectedPlaylist == playlist.name ? "▶" : " ")
                                    .foregroundStyle(RustMusicPalette.phosphor)
                                Text(playlist.name).lineLimit(1)
                                Spacer()
                                Text("\(playlist.paths.count)")
                                    .foregroundStyle(RustMusicPalette.muted)
                            }
                            .frame(maxWidth: .infinity, alignment: .leading)
                        }
                        .buttonStyle(.plain)
                        .listRowBackground(RustMusicPalette.display)
                    }
                }
                .listStyle(.plain)
                .scrollContentBackground(.hidden)
                .background(RustMusicPalette.display)
                .frame(height: 112)
                .disabled(model.isTransportBusy || model.isStateBusy)

                if let selectedPlaylist = model.playlists.first(where: { $0.name == model.selectedPlaylist }) {
                    Text("TRACKS · \(selectedPlaylist.name)")
                        .font(.system(size: 10, weight: .medium, design: .monospaced))
                        .foregroundStyle(RustMusicPalette.muted)
                    List(selection: Binding<Set<Int>>(
                        get: { model.selectedPlaylistEntryIDs },
                        set: { model.selectedPlaylistEntryIDs = $0 }
                    )) {
                        ForEach(Array(selectedPlaylist.paths.enumerated()), id: \.offset) { index, path in
                            HStack(spacing: 6) {
                                Text(model.tracks.first(where: { $0.relativePath == path })?.title
                                    ?? URL(fileURLWithPath: path).lastPathComponent)
                                    .lineLimit(1)
                            }
                            .tag(index)
                            .listRowBackground(RustMusicPalette.display)
                            .accessibilityLabel("Playlist track \(index + 1): \(URL(fileURLWithPath: path).lastPathComponent)")
                        }
                    }
                    .listStyle(.plain)
                    .scrollContentBackground(.hidden)
                    .background(RustMusicPalette.display)
                    .frame(height: 140)
                    .disabled(model.isTransportBusy || model.isStateBusy)
                    .accessibilityLabel("Tracks in selected playlist")
                    .accessibilityHint("Select one or more tracks, then Tab to reorder or remove actions.")
                    .accessibilityIdentifier("playlist-entry-list")

                    HStack(spacing: 6) {
                        Button("MOVE TRACKS UP") { model.moveSelectedPlaylistEntries(by: -1) }
                            .disabled(model.selectedPlaylistEntryIDs.isEmpty || model.selectedPlaylistEntryIDs.allSatisfy { $0 == 0 } || model.selectedPlaylistEntryIDs.count > 64 || model.isStateBusy || model.isTransportBusy)
                        Button("MOVE TRACKS DOWN") { model.moveSelectedPlaylistEntries(by: 1) }
                            .disabled(model.selectedPlaylistEntryIDs.isEmpty || model.selectedPlaylistEntryIDs.allSatisfy { $0 == selectedPlaylist.paths.count - 1 } || model.selectedPlaylistEntryIDs.count > 64 || model.isStateBusy || model.isTransportBusy)
                        Button("REMOVE TRACKS · \(model.selectedPlaylistEntryIDs.count)") { model.removeSelectedPlaylistEntries() }
                            .disabled(model.selectedPlaylistEntryIDs.isEmpty || model.selectedPlaylistEntryIDs.count > 64 || model.isStateBusy || model.isTransportBusy)
                    }
                }

                HStack(spacing: 6) {
                    Button("CREATE") { model.createPlaylist() }
                        .disabled(model.newPlaylistName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || model.isStateBusy || model.isTransportBusy)
                    Button("RENAME") { model.renameSelectedPlaylist() }
                        .disabled(model.selectedPlaylist == nil || model.newPlaylistName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || model.isStateBusy || model.isTransportBusy)
                    Button("ADD SELECTED · \(model.selectedTrackIDs.count)") { model.addSelectionToPlaylist() }
                        .disabled(model.selectedPlaylist == nil || model.selectedTrackIDs.isEmpty || model.isStateBusy || model.isTransportBusy)
                }
                HStack(spacing: 6) {
                    Button("QUEUE PLAYLIST") { model.addSelectedPlaylistToQueue() }
                        .disabled(model.selectedPlaylist == nil || model.isStateBusy || model.isTransportBusy)
                    Button("DELETE") { model.deleteSelectedPlaylist() }
                        .disabled(model.selectedPlaylist == nil || model.isStateBusy || model.isTransportBusy)
                }
            }
        } label: {
            Text("PLAYLISTS · \(model.playlists.count)")
        }
        .groupBoxStyle(RustMusicGroupBoxStyle())
    }

    private var transportBar: some View {
        VStack(spacing: 8) {
            Rectangle().fill(RustMusicPalette.line).frame(height: 1)
            HStack(spacing: 7) {
                Button("PREVIOUS") { model.previous() }
                    .disabled((model.tracks.isEmpty && model.queueEntries.isEmpty) || model.isScanning || model.isStateBusy || model.isTransportBusy)
                    .accessibilityIdentifier("transport-previous")
                Button("PLAY") { model.play() }
                    .buttonStyle(RustMusicButtonStyle(prominent: true))
                    .disabled(model.selectedTrack == nil || model.isPlaying || model.isPaused || model.isScanning || model.isStateBusy || model.isTransportBusy)
                    .accessibilityIdentifier("transport-play")
                Button(model.isPaused ? "RESUME" : "PAUSE") { model.togglePause() }
                    .disabled((!model.isPlaying && !model.isPaused) || model.isTransportBusy || model.isStateBusy)
                    .accessibilityIdentifier("transport-pause-resume")
                Button("STOP") { model.stop() }
                    .disabled((!model.isPlaying && !model.isPaused) || model.isTransportBusy || model.isStateBusy)
                    .accessibilityIdentifier("transport-stop")
                Button("NEXT") { model.next() }
                    .disabled((model.tracks.isEmpty && model.queueEntries.isEmpty) || model.isScanning || model.isStateBusy || model.isTransportBusy)
                    .accessibilityIdentifier("transport-next")
                Spacer(minLength: 14)
                Text("VOLUME \(model.volumePercent)%")
                    .font(.system(size: 10, weight: .semibold, design: .monospaced))
                    .foregroundStyle(RustMusicPalette.muted)
                Slider(
                    value: Binding(
                        get: { Double(model.volumePercent) },
                        set: { model.volumePercent = Int($0.rounded()) }
                    ),
                    in: 0...100,
                    step: 1,
                    onEditingChanged: { editing in
                        if !editing { model.commitVolume(model.volumePercent) }
                    }
                )
                .frame(width: 120)
                .disabled(model.libraryRoot == nil || model.isTransportBusy || model.isStateBusy)
                .accessibilityIdentifier("transport-volume")
            }
            .buttonStyle(RustMusicButtonStyle())
            .padding(.horizontal, 16)
            .padding(.bottom, 10)
        }
        .background(RustMusicPalette.surface)
        .accessibilityIdentifier("transport-bar")
    }
}

/// A lightweight, native SwiftUI chassis that frames the functional player.
/// It uses only static shapes and text; it contains no artwork or audio meter.
private struct PipBoyChassis<Content: View>: View {
    private let content: Content

    init(@ViewBuilder content: () -> Content) {
        self.content = content()
    }

    var body: some View {
        VStack(spacing: 0) {
            HStack(alignment: .center, spacing: 10) {
                Text("RUST MUSIC")
                    .font(.system(size: 12, weight: .black, design: .monospaced))
                    .tracking(1.4)
                    .foregroundStyle(RustMusicPalette.brassHighlight)
                Rectangle()
                    .fill(RustMusicPalette.brass)
                    .frame(height: 1)
                Text("PIP-BOY 2000 · AUDIO")
                    .font(.system(size: 9, weight: .bold, design: .monospaced))
                    .tracking(0.9)
                    .foregroundStyle(RustMusicPalette.muted)
            }
            .padding(.horizontal, 18)
            .padding(.top, 12)
            .padding(.bottom, 8)

            content
                .frame(maxWidth: .infinity, maxHeight: .infinity)
                .background(RustMusicPalette.background, in: RoundedRectangle(cornerRadius: 10))
                .overlay {
                    RoundedRectangle(cornerRadius: 10)
                        .stroke(RustMusicPalette.line.opacity(0.8), lineWidth: 1)
                }
                .padding(.horizontal, 8)
                .padding(.bottom, 8)
        }
        .background {
            RoundedRectangle(cornerRadius: 16)
                .fill(LinearGradient(
                    colors: [RustMusicPalette.brassHighlight.opacity(0.82), RustMusicPalette.brass, Color(red: 0.13, green: 0.12, blue: 0.09)],
                    startPoint: .topLeading,
                    endPoint: .bottomTrailing
                ))
                .overlay {
                    ZStack {
                        RoundedRectangle(cornerRadius: 16)
                            .stroke(RustMusicPalette.brassHighlight.opacity(0.75), lineWidth: 1)
                        RoundedRectangle(cornerRadius: 13)
                            .stroke(RustMusicPalette.brass.opacity(0.85), lineWidth: 1)
                            .padding(3)
                    }
                }
        }
        .overlay {
            RoundedRectangle(cornerRadius: 16)
                .stroke(Color.black.opacity(0.55), lineWidth: 1)
        }
        .overlay(alignment: .topLeading) { ChassisFastener().padding(9) }
        .overlay(alignment: .topTrailing) { ChassisFastener().padding(9) }
        .overlay(alignment: .bottomLeading) { ChassisFastener().padding(9) }
        .overlay(alignment: .bottomTrailing) { ChassisFastener().padding(9) }
        .padding(10)
    }
}

private struct ChassisFastener: View {
    var body: some View {
        ZStack {
            Circle().fill(RustMusicPalette.background.opacity(0.9))
            Circle().stroke(RustMusicPalette.brassHighlight.opacity(0.8), lineWidth: 1)
            Rectangle()
                .fill(RustMusicPalette.brassHighlight.opacity(0.8))
                .frame(width: 7, height: 1)
                .rotationEffect(.degrees(-35))
        }
        .frame(width: 8, height: 8)
        .accessibilityHidden(true)
    }
}

private struct RustMusicTextFieldStyle: ViewModifier {
    func body(content: Content) -> some View {
        content
            .textFieldStyle(.plain)
            .font(.system(size: 11, weight: .regular, design: .monospaced))
            .foregroundStyle(RustMusicPalette.phosphor)
            .padding(.horizontal, 8)
            .padding(.vertical, 7)
            .background(RustMusicPalette.display, in: RoundedRectangle(cornerRadius: 4))
            .overlay {
                RoundedRectangle(cornerRadius: 4)
                    .stroke(RustMusicPalette.line, lineWidth: 1)
            }
    }
}

private struct RustMusicButtonStyle: ButtonStyle {
    var prominent = false

    func makeBody(configuration: Configuration) -> some View {
        configuration.label
            .font(.system(size: 10, weight: .bold, design: .monospaced))
            .tracking(0.35)
            .foregroundStyle(prominent ? RustMusicPalette.background : RustMusicPalette.phosphor)
            .padding(.horizontal, 9)
            .padding(.vertical, 7)
            .background(
                prominent ? RustMusicPalette.phosphor : RustMusicPalette.surface,
                in: RoundedRectangle(cornerRadius: 4)
            )
            .overlay {
                RoundedRectangle(cornerRadius: 4)
                    .stroke(prominent ? RustMusicPalette.phosphor : RustMusicPalette.line, lineWidth: 1)
            }
            .opacity(configuration.isPressed ? 0.78 : 1)
    }
}

private struct RustMusicGroupBoxStyle: GroupBoxStyle {
    func makeBody(configuration: Configuration) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            configuration.label
                .font(.system(size: 10, weight: .bold, design: .monospaced))
                .tracking(1.15)
                .foregroundStyle(RustMusicPalette.phosphor)
            configuration.content
        }
        .padding(10)
        .background(RustMusicPalette.surface, in: RoundedRectangle(cornerRadius: 6))
        .overlay {
            RoundedRectangle(cornerRadius: 6)
                .stroke(RustMusicPalette.line, lineWidth: 1)
        }
    }
}
