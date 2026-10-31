import AppKit
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

struct RustMusicFailure: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}

struct CatalogTrack: Identifiable, Equatable {
    let id: Int
    let container: String
    let title: String
    let artist: String
    let album: String
    let relativePath: String
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
        }
        var payload = Data([1, 1]) // RBPK v1 request envelope
        payload.appendBE(requestID)
        try payload.appendText("org.rustboy.music", max: 96)
        payload.append(body)
        guard payload.count <= 16_384 else { throw RustMusicFailure(message: "Package request exceeds the frame limit.") }
        return payload
    }
}

private struct PackageReader {
    let bytes: [UInt8]
    var offset = 0

    mutating func take(_ count: Int) throws -> [UInt8] {
        guard count >= 0, count <= 16_384, offset <= bytes.count, count <= bytes.count - offset else {
            throw RustMusicFailure(message: "Rust Music returned a truncated package frame.")
        }
        defer { offset += count }
        return Array(bytes[offset..<(offset + count)])
    }

    mutating func byte() throws -> UInt8 { try take(1)[0] }
    mutating func u16() throws -> UInt16 {
        let value = try take(2)
        return (UInt16(value[0]) << 8) | UInt16(value[1])
    }
    mutating func u32() throws -> UInt32 {
        let value = try take(4)
        return value.reduce(UInt32(0)) { ($0 << 8) | UInt32($1) }
    }
    mutating func u64() throws -> UInt64 {
        let value = try take(8)
        return value.reduce(UInt64(0)) { ($0 << 8) | UInt64($1) }
    }
    mutating func text(max: Int) throws -> String {
        let count = Int(try u16())
        guard count <= max else { throw RustMusicFailure(message: "Rust Music returned oversized package text.") }
        guard let value = String(bytes: try take(count), encoding: .utf8),
              !value.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) }) else {
            throw RustMusicFailure(message: "Rust Music returned invalid package text.")
        }
        return value
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
    private var requestID: UInt64 = 1

    init(executable: URL, root: URL) throws {
        let process = Process()
        process.executableURL = executable
        process.arguments = ["serve", "--desktop"]
        let inputPipe = Pipe()
        let outputPipe = Pipe()
        process.standardInput = inputPipe
        process.standardOutput = outputPipe
        process.standardError = FileHandle.nullDevice
        try process.run()
        self.process = process
        input = inputPipe.fileHandleForWriting
        output = outputPipe.fileHandleForReading
        do {
            guard case .hello(let version) = try request(.hello), version == 1 else {
                throw RustMusicFailure(message: "Rust Music package protocol negotiation failed.")
            }
            if case .failure(let message) = try request(.activate) { throw RustMusicFailure(message: message) }
            if case .failure(let message) = try request(.scanLibrary(root.path)) { throw RustMusicFailure(message: message) }
        } catch {
            shutdown(force: true)
            throw error
        }
    }

    func request(_ body: PackageRequest) throws -> PackageResponse {
        guard process.isRunning else { throw RustMusicFailure(message: "Rust Music playback process has exited.") }
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
        try input.write(contentsOf: frame)
        let header = try readExact(8)
        guard Array(header.prefix(4)) == [0x52, 0x42, 0x50, 0x4b] else {
            throw RustMusicFailure(message: "Rust Music returned an invalid package frame header.")
        }
        var lengthReader = PackageReader(bytes: Array(header.dropFirst(4)))
        let length = Int(try lengthReader.u32())
        guard (1...16_384).contains(length) else { throw RustMusicFailure(message: "Rust Music package frame length is invalid.") }
        var reader = PackageReader(bytes: Array(try readExact(length)))
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
            let state = try reader.byte()
            guard state <= 6 else { throw RustMusicFailure(message: "Rust Music returned an unknown playback state.") }
            let selectedPresent = try reader.byte()
            guard selectedPresent <= 1 else { throw RustMusicFailure(message: "Rust Music returned an invalid selection flag.") }
            if selectedPresent == 1 { _ = try reader.u16() }
            _ = try reader.u16() // track count
            _ = try reader.u16() // page offset
            let count = Int(try reader.byte())
            guard count <= 32 else { throw RustMusicFailure(message: "Rust Music returned too many package tracks.") }
            for _ in 0..<count {
                _ = try reader.u16(); _ = try reader.text(max: 128); _ = try reader.byte()
            }
            let formatPresent = try reader.byte()
            guard formatPresent <= 1 else { throw RustMusicFailure(message: "Rust Music returned an invalid PCM format flag.") }
            if formatPresent == 1 { _ = try reader.u32(); _ = try reader.byte(); _ = try reader.byte() }
            _ = try reader.u64()
            let totalPresent = try reader.byte()
            guard totalPresent <= 1 else { throw RustMusicFailure(message: "Rust Music returned an invalid frame-count flag.") }
            if totalPresent == 1 { _ = try reader.u64() }
            guard try reader.byte() <= 1 else { throw RustMusicFailure(message: "Rust Music returned an invalid audio-claim flag.") }
            reply = .view(state)
        case 3:
            _ = try reader.byte() // error code
            reply = .failure(try reader.text(max: 256))
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
        if !waitForExit(timeout: 2) {
            process.terminate()
            if !waitForExit(timeout: 1) {
                _ = kill(process.processIdentifier, SIGKILL)
                process.waitUntilExit()
            }
        }
    }

    private func waitForExit(timeout: TimeInterval) -> Bool {
        let deadline = Date().addingTimeInterval(timeout)
        while process.isRunning && Date() < deadline {
            Thread.sleep(forTimeInterval: 0.01)
        }
        return !process.isRunning
    }

    private func readExact(_ count: Int) throws -> Data {
        var data = Data()
        data.reserveCapacity(count)
        while data.count < count {
            guard let chunk = try output.read(upToCount: count - data.count), !chunk.isEmpty else {
                throw RustMusicFailure(message: "Rust Music playback process closed its package stream.")
            }
            data.append(chunk)
        }
        return data
    }
}

private enum PackageResponse {
    case hello(UInt16)
    case ack
    case view(UInt8)
    case failure(String)
}

@main
struct RustMusicApp: App {
    var body: some Scene {
        WindowGroup {
            PlayerView()
                .frame(minWidth: 600, minHeight: 390)
                .background(RustMusicPalette.background)
                .preferredColorScheme(.dark)
                .tint(RustMusicPalette.phosphor)
        }
        .windowResizability(.contentSize)
    }
}

@MainActor
final class PlayerModel: ObservableObject {
    @Published var libraryRoot: URL?
    @Published var tracks: [CatalogTrack] = []
    @Published var selectedIndex: Int?
    @Published var searchText = ""
    @Published var status = "Choose a local library folder."
    @Published var isPlaying = false
    @Published var isPaused = false
    @Published var isTransportBusy = false
    @Published var volumePercent = 100
    @Published var isScanning = false
    @Published var isStateBusy = false
    @Published var queueEntries: [QueueEntry] = []
    @Published var queueSelection: Int?
    @Published var selectedQueueIndex: Int?
    @Published var repeatMode = "off"
    @Published var shuffleSeed: String?
    @Published var playlists: [UserPlaylist] = []
    @Published var selectedPlaylist: String?
    @Published var newPlaylistName = ""

    private var scanCancellationRequested = false
    private var scanProcess: Process?
    private var stateProcess: Process?
    private let packageQueue = DispatchQueue(label: "dev.rustmusic.package-process")
    nonisolated(unsafe) private var packageClient: PackageProcessClient?
    nonisolated(unsafe) private var packageRootPath: String?
    private var snapshotPollScheduled = false
    private var pendingStateReload = false

    private var decoderURL: URL {
        Bundle.main.bundleURL.appendingPathComponent("Contents/MacOS/rust-music-cli")
    }

    var visibleTracks: [CatalogTrack] {
        let query = searchText.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return tracks }
        return tracks.filter {
            $0.title.localizedCaseInsensitiveContains(query)
                || $0.artist.localizedCaseInsensitiveContains(query)
                || $0.album.localizedCaseInsensitiveContains(query)
                || $0.relativePath.localizedCaseInsensitiveContains(query)
        }
    }

    var selectedTrack: CatalogTrack? {
        guard let selectedIndex else { return nil }
        return tracks.first(where: { $0.id == selectedIndex })
    }

    func boundSearchText() {
        var bounded = ""
        for scalar in searchText.unicodeScalars {
            let value = scalar.value
            if value < 0x20 || (0x7f...0x9f).contains(value) { continue }
            let scalarBytes = String(scalar).utf8.count
            if bounded.utf8.count + scalarBytes > 512 { break }
            bounded.unicodeScalars.append(scalar)
        }
        if bounded != searchText { searchText = bounded }
        // Filtering never changes the selected playback target. A playing
        // track may be outside the current search results without switching.
    }

    private func runStateCommand(
        _ arguments: [String],
        completion: @escaping (Result<Data, RustMusicFailure>) -> Void
    ) {
        guard libraryRoot != nil, !isStateBusy, !isTransportBusy else { return }
        let process = Process()
        process.executableURL = decoderURL
        process.arguments = ["state"] + arguments
        let output = Pipe()
        process.standardOutput = output
        process.standardError = output
        do {
            try process.run()
        } catch {
            completion(.failure(RustMusicFailure(
                message: "Could not start Rust Music state storage: \(error.localizedDescription)"
            )))
            return
        }
        stateProcess = process
        isStateBusy = true
        Task { [weak self] in
            let (bytes, exitStatus) = await Task.detached(priority: .userInitiated) {
                let bytes = output.fileHandleForReading.readDataToEndOfFile()
                process.waitUntilExit()
                return (bytes, process.terminationStatus)
            }.value
            guard let self, self.stateProcess === process else { return }
            self.stateProcess = nil
            self.isStateBusy = false
            if exitStatus == 0 {
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
            case .failure(let failure):
                self.status = failure.message
            }
        }
    }

    func addSelectionToQueue() {
        guard let libraryRoot, let selectedTrack else { return }
        runStateCommand(["queue", libraryRoot.path, "add", selectedTrack.relativePath]) { [weak self] result in
            self?.applyQueueResult(result)
        }
    }

    func removeQueueSelection() {
        guard let libraryRoot, let selectedQueueIndex else { return }
        runStateCommand(["queue", libraryRoot.path, "remove", String(selectedQueueIndex)]) { [weak self] result in
            self?.applyQueueResult(result)
        }
    }

    func selectQueueEntry(_ index: Int) {
        guard let libraryRoot else { return }
        selectedQueueIndex = index
        runStateCommand(["queue", libraryRoot.path, "select", String(index)]) { [weak self] result in
            self?.applyQueueResult(result)
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
            self?.applyPlaylistResult(result)
            if case .success = result { self?.newPlaylistName = "" }
        }
    }

    func renameSelectedPlaylist() {
        guard let libraryRoot, let selectedPlaylist else { return }
        let name = newPlaylistName.trimmingCharacters(in: .whitespacesAndNewlines)
        guard !name.isEmpty, name.utf8.count <= 128 else { return }
        runStateCommand(["playlist", libraryRoot.path, "rename", selectedPlaylist, name]) { [weak self] result in
            self?.applyPlaylistResult(result)
            if case .success = result {
                self?.selectedPlaylist = name
                self?.newPlaylistName = ""
            }
        }
    }

    func addSelectionToPlaylist() {
        guard let libraryRoot, let selectedTrack, let selectedPlaylist else { return }
        runStateCommand(["playlist", libraryRoot.path, "add", selectedPlaylist, selectedTrack.relativePath]) { [weak self] result in
            self?.applyPlaylistResult(result)
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
        finishedAtEnd: Bool = false
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
                if continuePlayback { restartQueuePlayback() }
                else if selectionChanged && (isPlaying || isPaused) { stop() }
            }
            status = "Queue saved. Selection does not start playback; press Play explicitly."
        case .failure(let failure):
            status = failure.message
        }
    }

    private func applyPlaylistResult(_ result: Result<Data, RustMusicFailure>) {
        switch result {
        case .success(let bytes):
            guard let playlists = Self.parsePlaylists(bytes) else {
                status = "Rust Music returned malformed playlist state."
                return
            }
            self.playlists = playlists
            status = "Playlists saved. Source audio remains unchanged."
        case .failure(let failure):
            status = failure.message
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
        guard !isScanning, !isStateBusy, !isTransportBusy else { return }
        stop()
        let process = Process()
        process.executableURL = decoderURL
        process.arguments = ["catalog", root.path]
        let output = Pipe()
        process.standardOutput = output
        process.standardError = output
        do {
            try process.run()
        } catch {
            status = "Could not start the Rust Music catalog: \(error.localizedDescription)"
            return
        }
        scanProcess = process
        isScanning = true
        scanCancellationRequested = false
        status = "Scanning the selected folder… playback will not start."
        Task { [weak self] in
            let (bytes, outputFailure, exitStatus) = await Task.detached(priority: .userInitiated) {
                let bytes: Data
                let outputFailure: String?
                do {
                    if let bounded = try CatalogValidation.readBounded(
                        output.fileHandleForReading,
                        maximumBytes: CatalogValidation.maximumOutputBytes
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
                if outputFailure != nil && process.isRunning {
                    process.terminate()
                }
                process.waitUntilExit()
                return (bytes, outputFailure, process.terminationStatus)
            }.value
            guard let self, self.scanProcess === process else { return }
            let result: Result<[CatalogTrack], RustMusicFailure>
            if let outputFailure {
                result = .failure(RustMusicFailure(message: outputFailure))
            } else if exitStatus == 0 {
                result = Self.parseCatalog(bytes)
            } else {
                let message = String(data: bytes, encoding: .utf8)?
                    .trimmingCharacters(in: .whitespacesAndNewlines)
                result = .failure(RustMusicFailure(
                    message: message?.isEmpty == false ? message ?? "" : "The catalog scan failed."
                ))
            }
            self.scanProcess = nil
            self.isScanning = false
            let wasCancelled = self.scanCancellationRequested
            self.scanCancellationRequested = false
            switch result {
            case .success(let tracks):
                self.libraryRoot = root
                self.tracks = tracks
                self.selectedIndex = tracks.first?.id
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
        let volume = UInt8(volumePercent)
        runPackageCommands([.select(index), .setVolumePercent(volume), .play], label: "Starting playback…") { [weak self] response in
            self?.applyPlaybackResponse(response, fallback: "Playback started.")
        }
    }

    private func restartQueuePlayback() {
        guard let selectedTrack,
              let index = UInt16(exactly: selectedTrack.id) else { return }
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

    private func runPackageCommands(
        _ requests: [PackageRequest],
        label: String,
        completion: @escaping (PackageResponse) -> Void
    ) {
        guard let root = libraryRoot, !isTransportBusy, !isStateBusy else { return }
        isTransportBusy = true
        status = label
        let executable = decoderURL
        packageQueue.async { [weak self] in
            guard let self else { return }
            do {
                let rootPath = root.standardizedFileURL.path
                if self.packageRootPath != rootPath {
                    self.packageClient?.shutdown()
                    self.packageClient = try PackageProcessClient(executable: executable, root: root)
                    self.packageRootPath = rootPath
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
                DispatchQueue.main.async {
                    self.finishTransportOperation()
                    self.isPlaying = false
                    self.isPaused = false
                    self.status = "Rust Music playback failed: \(error.localizedDescription)"
                }
            }
        }
    }

    private func applyPlaybackResponse(
        _ response: PackageResponse,
        fallback: String,
        retryStateAfterError: Bool = true
    ) {
        switch response {
        case .view(let state):
            let flags = PlaybackUIState.flags(for: state)
            isPlaying = flags.playing
            isPaused = flags.paused
            if state != 2 { snapshotPollScheduled = false }
            switch state {
            case 2: status = "Playing through Rust Music and macOS AudioQueue."
            case 6: status = "Paused. Audio focus is released until Resume is requested."
            case 3: status = "Playback stopped."
            case 4: status = "Playback finished."
            case 5: status = "Playback failed."
            default: status = fallback
            }
            // A failed native output must release the UI's transport lockout
            // so the user can explicitly retry after restoring the device.
            if state == 5 {
                isPlaying = false
                isPaused = false
                snapshotPollScheduled = false
            }
            if isPlaying { scheduleSnapshotPoll() }
        case .failure(let message):
            let shouldReconcileState = isPlaying || isPaused
            status = message
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
        guard !snapshotPollScheduled else { return }
        snapshotPollScheduled = true
        DispatchQueue.main.asyncAfter(deadline: .now() + 0.5) { [weak self] in
            guard let self else { return }
            self.snapshotPollScheduled = false
            guard self.isPlaying, !self.isTransportBusy else { return }
            self.runPackageCommands([.snapshot(0)], label: "") { [weak self] response in
                guard let self else { return }
                if case .view(let state) = response, state == 4 {
                    self.isPlaying = false
                    self.advanceQueueAfterEnd()
                    return
                }
                self.applyPlaybackResponse(response, fallback: "Playback active.")
                if self.isPlaying { self.scheduleSnapshotPoll() }
            }
        }
    }

    func shutdownPackage() {
        cancelScan(); cancelStateOperation()
        isTransportBusy = true
        // Retain the model through the serialized cleanup. A weak capture here
        // can drop the Shutdown/Stop request if SwiftUI releases the view model
        // immediately after the window closes.
        packageQueue.async { [self] in
            self.packageClient?.shutdown()
            self.packageClient = nil
            self.packageRootPath = nil
            DispatchQueue.main.async {
                self.isTransportBusy = false
                self.isPlaying = false
                self.isPaused = false
            }
        }
    }

    func selectTrackManually(_ id: Int?) {
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

private enum PlayerSection: String, CaseIterable, Identifiable {
    case nowPlaying
    case library
    case queue
    case playlists

    var id: String { rawValue }
    var title: String {
        switch self {
        case .nowPlaying: return "PLAYER"
        case .library: return "LIBRARY"
        case .queue: return "QUEUE"
        case .playlists: return "PLAYLISTS"
        }
    }
}

struct PlayerView: View {
    @StateObject private var model = PlayerModel()
    @State private var activeSection: PlayerSection = .nowPlaying

    var body: some View {
        PipBoyChassis {
            VStack(spacing: 0) {
                ScrollView {
                    VStack(alignment: .leading, spacing: 12) {
                        header
                        HStack(alignment: .top, spacing: 12) {
                            navigationRail
                                .frame(width: 112)
                            sectionContent
                                .frame(maxWidth: .infinity, alignment: .topLeading)
                        }
                    }
                    .padding(12)
                    .frame(minWidth: 600, alignment: .leading)
                }
                .scrollIndicators(.visible)

                transportBar
            }
        }
        .background(RustMusicPalette.background.ignoresSafeArea())
        .foregroundStyle(RustMusicPalette.phosphor)
        .preferredColorScheme(.dark)
        .tint(RustMusicPalette.phosphor)
        .onChange(of: model.searchText) { _ in model.boundSearchText() }
        .onDisappear { model.shutdownPackage() }
    }

    private var navigationRail: some View {
        VStack(alignment: .leading, spacing: 7) {
            ForEach(PlayerSection.allCases) { section in
                Button {
                    activeSection = section
                } label: {
                    HStack(spacing: 6) {
                        Text(section == activeSection ? "▶" : " ")
                            .frame(width: 10)
                        Text(section.title)
                            .lineLimit(1)
                            .minimumScaleFactor(0.8)
                        Spacer(minLength: 0)
                    }
                    .frame(maxWidth: .infinity, alignment: .leading)
                }
                .buttonStyle(RustMusicButtonStyle(prominent: section == activeSection))
                .accessibilityIdentifier("navigation-\(section.rawValue)")
            }
            Spacer(minLength: 0)
        }
        .padding(8)
        .background(RustMusicPalette.surface, in: RoundedRectangle(cornerRadius: 6))
        .overlay {
            RoundedRectangle(cornerRadius: 6)
                .stroke(RustMusicPalette.line, lineWidth: 1)
        }
        .accessibilityIdentifier("navigation-rail")
    }

    @ViewBuilder
    private var sectionContent: some View {
        switch activeSection {
        case .nowPlaying:
            nowPlayingPanel
        case .library:
            libraryPanel
        case .queue:
            queuePanel
        case .playlists:
            playlistPanel
        }
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
                .frame(height: activeSection == .queue ? 220 : 86)
                .disabled(model.isTransportBusy || model.isStateBusy)

                HStack(spacing: 6) {
                    Button("ADD") { model.addSelectionToQueue() }
                        .disabled(model.selectedTrack == nil || model.isStateBusy || model.isTransportBusy)
                    Button("REMOVE") { model.removeQueueSelection() }
                        .disabled(model.selectedQueueIndex == nil || model.isStateBusy || model.isTransportBusy)
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
                    Text("\(visibleTracks.count) TRACKS")
                        .font(.system(size: 10, weight: .medium, design: .monospaced))
                        .foregroundStyle(RustMusicPalette.muted)
                        .fixedSize()
                }

                List(selection: Binding(
                    get: { model.selectedIndex },
                    set: { model.selectTrackManually($0) }
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
                .frame(height: activeSection == .playlists ? 220 : 112)
                .disabled(model.isTransportBusy || model.isStateBusy)

                HStack(spacing: 6) {
                    Button("CREATE") { model.createPlaylist() }
                        .disabled(model.newPlaylistName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || model.isStateBusy || model.isTransportBusy)
                    Button("RENAME") { model.renameSelectedPlaylist() }
                        .disabled(model.selectedPlaylist == nil || model.newPlaylistName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || model.isStateBusy || model.isTransportBusy)
                    Button("ADD TRACK") { model.addSelectionToPlaylist() }
                        .disabled(model.selectedPlaylist == nil || model.selectedTrack == nil || model.isStateBusy || model.isTransportBusy)
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
                    .keyboardShortcut(.space, modifiers: [])
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
