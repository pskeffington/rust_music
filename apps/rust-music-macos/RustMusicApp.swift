import AppKit
import Darwin
import SwiftUI
import UniformTypeIdentifiers

private enum RustMusicPalette {
    static let background = Color(red: 0.025, green: 0.055, blue: 0.035)
    static let surface = Color(red: 0.055, green: 0.105, blue: 0.060)
    static let phosphor = Color(red: 0.55, green: 0.96, blue: 0.48)
    static let muted = Color(red: 0.54, green: 0.68, blue: 0.49)
    static let line = Color(red: 0.20, green: 0.36, blue: 0.22)
}

struct RustMusicFailure: Error {
    let message: String
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
    private var playProcess: Process?
    private var parentGuardPipe: Pipe?
    private var escalationScheduledFor: Int32?
    private var stopRequested = false

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
        let selectedIsVisible = selectedIndex.map { index in
            visibleTracks.contains(where: { $0.id == index })
        } ?? false
        if !selectedIsVisible {
            selectedIndex = visibleTracks.first?.id
        }
    }

    private func runStateCommand(
        _ arguments: [String],
        completion: @escaping (Result<Data, RustMusicFailure>) -> Void
    ) {
        guard libraryRoot != nil, !isStateBusy else { return }
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
        reloadQueue()
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
        guard !queueEntries.isEmpty, let libraryRoot else {
            moveSelection(forward: true)
            return
        }
        runStateCommand(["queue", libraryRoot.path, "next"]) { [weak self] result in
            self?.applyQueueResult(result)
        }
    }

    func previous() {
        guard !queueEntries.isEmpty, let libraryRoot else {
            moveSelection(forward: false)
            return
        }
        runStateCommand(["queue", libraryRoot.path, "previous"]) { [weak self] result in
            self?.applyQueueResult(result)
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

    private func applyQueueResult(_ result: Result<Data, RustMusicFailure>) {
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
            if let current = queue.current,
               let entry = queue.entries.first(where: { $0.index == current }),
               let track = tracks.first(where: { $0.relativePath == entry.relativePath }) {
                selectedIndex = track.id
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
        guard !isScanning, !isStateBusy else { return }
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
            let (bytes, exitStatus) = await Task.detached(priority: .userInitiated) {
                let bytes = output.fileHandleForReading.readDataToEndOfFile()
                process.waitUntilExit()
                return (bytes, process.terminationStatus)
            }.value
            guard let self, self.scanProcess === process else { return }
            let result: Result<[CatalogTrack], RustMusicFailure>
            if exitStatus == 0 {
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
        guard let libraryRoot, let selectedTrack, playProcess == nil, !isScanning else { return }
        let process = Process()
        process.executableURL = decoderURL
        process.arguments = ["play-managed-root", libraryRoot.path, selectedTrack.relativePath]
        let parentGuard = Pipe()
        process.standardInput = parentGuard.fileHandleForReading
        let errors = Pipe()
        process.standardError = errors
        process.terminationHandler = { [weak self] child in
            let message = errors.fileHandleForReading.readDataToEndOfFile()
            let errorText = String(data: message, encoding: .utf8)?
                .trimmingCharacters(in: .whitespacesAndNewlines) ?? ""
            Task { @MainActor in
                guard let self, self.playProcess === child else { return }
                self.parentGuardPipe?.fileHandleForWriting.closeFile()
                self.parentGuardPipe = nil
                self.escalationScheduledFor = nil
                self.playProcess = nil
                self.isPlaying = false
                if child.terminationStatus == 0 {
                    self.status = self.stopRequested ? "Playback stopped." : "Playback finished."
                } else {
                    self.status = errorText.isEmpty ? "Playback stopped." : errorText
                }
                self.stopRequested = false
            }
        }
        playProcess = process
        parentGuardPipe = parentGuard
        stopRequested = false
        do {
            try process.run()
            isPlaying = true
            status = "Playing through Rust Music and macOS AudioQueue."
        } catch {
            parentGuard.fileHandleForWriting.closeFile()
            parentGuardPipe = nil
            playProcess = nil
            status = "Could not start Rust Music: \(error.localizedDescription)"
        }
    }

    func stop() {
        guard let playProcess else { return }
        stopRequested = true
        if let parentGuardPipe {
            parentGuardPipe.fileHandleForWriting.closeFile()
            self.parentGuardPipe = nil
        }
        status = "Stopping Rust Music playback…"
        let processID = playProcess.processIdentifier
        guard escalationScheduledFor != processID else { return }
        escalationScheduledFor = processID
        DispatchQueue.main.asyncAfter(deadline: .now() + 5) { [weak self, weak playProcess] in
            guard let self, let playProcess,
                  self.playProcess === playProcess,
                  playProcess.isRunning else { return }
            playProcess.terminate()
            DispatchQueue.main.asyncAfter(deadline: .now() + 2) { [weak self, weak playProcess] in
                guard let self, let playProcess,
                      self.playProcess === playProcess,
                      playProcess.isRunning else { return }
                _ = kill(playProcess.processIdentifier, SIGKILL)
            }
        }
    }

    func selectionChanged() {
        if isPlaying { stop() }
    }

    private func moveSelection(forward: Bool) {
        guard !tracks.isEmpty else { return }
        let current = selectedIndex.flatMap { id in tracks.firstIndex(where: { $0.id == id }) }
        let next: Int
        if forward {
            next = min((current ?? -1) + 1, tracks.count - 1)
        } else {
            next = max((current ?? tracks.count) - 1, 0)
        }
        selectedIndex = tracks[next].id
        status = "Selected \(tracks[next].title). Press Play to start output."
    }

    nonisolated private static func parseCatalog(_ data: Data) -> Result<[CatalogTrack], RustMusicFailure> {
        guard let output = String(data: data, encoding: .utf8) else {
            return .failure(RustMusicFailure(message: "Rust Music returned invalid catalog text."))
        }
        var tracks: [CatalogTrack] = []
        var reportedCount: Int?
        for line in output.split(whereSeparator: \.isNewline) {
            let fields = line.split(separator: "\t", omittingEmptySubsequences: false)
            if fields.first == "summary" {
                guard fields.count == 4,
                      let count = Int(fields[1]), count >= 0,
                      Int(fields[2]) != nil, Int(fields[3]) != nil,
                      reportedCount == nil else {
                    return .failure(RustMusicFailure(message: "Rust Music returned a malformed catalog summary."))
                }
                reportedCount = count
                continue
            }
            guard fields.first == "track", fields.count == 7,
                  let index = Int(fields[1]), index >= 0,
                  !tracks.contains(where: { $0.id == index }),
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
        return ParsedQueue(entries: entries, repeatMode: repeatMode, shuffleSeed: shuffleSeed, current: current)
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

struct PlayerView: View {
    @StateObject private var model = PlayerModel()

    var body: some View {
        VStack(alignment: .leading, spacing: 16) {
            HStack(alignment: .top, spacing: 14) {
                Text("RM")
                    .font(.system(size: 28, weight: .medium))
                    .foregroundStyle(RustMusicPalette.phosphor)
                    .frame(width: 48, height: 48)
                    .background(RustMusicPalette.surface, in: RoundedRectangle(cornerRadius: 12))
                VStack(alignment: .leading, spacing: 4) {
                    Text("Rust Music")
                        .font(.largeTitle.weight(.semibold))
                    Text("Text-only, offline library for lossless audio")
                        .foregroundStyle(RustMusicPalette.muted)
                }
                Spacer()
                Button("Load Library…") {
                    model.chooseLibrary()
                }
                .disabled(model.isScanning || model.isStateBusy)
                .keyboardShortcut("o", modifiers: .command)
            }

            HStack(spacing: 10) {
                TextField("Search title, artist, album or path", text: $model.searchText)
                    .textFieldStyle(.roundedBorder)
                    .disabled(model.tracks.isEmpty || model.isScanning)
                if model.isScanning {
                    Button("Cancel Scan") { model.cancelScan() }
                }
            }

            GroupBox("Library Tracks (\(model.visibleTracks.count))") {
                List(selection: $model.selectedIndex) {
                    ForEach(model.visibleTracks) { track in
                        VStack(alignment: .leading, spacing: 3) {
                            Text(track.title).font(.headline).lineLimit(1)
                            Text([track.artist, track.album].filter { !$0.isEmpty }.joined(separator: " · "))
                                .font(.caption)
                                .foregroundStyle(RustMusicPalette.muted)
                                .lineLimit(1)
                            Text("\(track.container) · \(track.relativePath)")
                                .font(.system(.caption2, design: .monospaced))
                                .foregroundStyle(RustMusicPalette.muted)
                                .lineLimit(1)
                        }
                        .tag(track.id)
                    }
                }
                .frame(minHeight: 130, maxHeight: 180)
            }
            .groupBoxStyle(RustMusicGroupBoxStyle())

            HStack(alignment: .top, spacing: 12) {
                GroupBox("Persistent Queue (\(model.queueEntries.count))") {
                    VStack(alignment: .leading, spacing: 6) {
                        List {
                            ForEach(model.queueEntries) { entry in
                                Button {
                                    model.selectQueueEntry(entry.index)
                                } label: {
                                    let track = model.tracks.first(where: { $0.relativePath == entry.relativePath })
                                    Text("\(entry.isSelected ? "Selected: " : "")\(track?.title ?? URL(fileURLWithPath: entry.relativePath).lastPathComponent)")
                                        .lineLimit(1)
                                        .frame(maxWidth: .infinity, alignment: .leading)
                                }
                                .buttonStyle(.plain)
                            }
                        }
                        .frame(height: 88)
                        HStack(spacing: 6) {
                            Button("Add Selected") { model.addSelectionToQueue() }
                                .disabled(model.selectedTrack == nil || model.isStateBusy)
                            Button("Remove") { model.removeQueueSelection() }
                                .disabled(model.selectedQueueIndex == nil || model.isStateBusy)
                        }
                        HStack(spacing: 6) {
                            Button("Repeat: \(model.repeatMode)") { model.cycleRepeatMode() }
                                .disabled(model.isStateBusy)
                            Button(model.shuffleSeed == nil ? "Shuffle" : "Shuffle: on") { model.toggleShuffle() }
                                .disabled(model.isStateBusy)
                        }
                    }
                }
                .groupBoxStyle(RustMusicGroupBoxStyle())

                GroupBox("Named Playlists (\(model.playlists.count))") {
                    VStack(alignment: .leading, spacing: 6) {
                        TextField("New playlist name", text: $model.newPlaylistName)
                            .textFieldStyle(.roundedBorder)
                        List {
                            ForEach(model.playlists) { playlist in
                                Button {
                                    model.selectedPlaylist = playlist.name
                                } label: {
                                    Text("\(model.selectedPlaylist == playlist.name ? "Selected: " : "")\(playlist.name) · \(playlist.paths.count)")
                                        .lineLimit(1)
                                        .frame(maxWidth: .infinity, alignment: .leading)
                                }
                                .buttonStyle(.plain)
                            }
                        }
                        .frame(height: 62)
                        HStack(spacing: 6) {
                            Button("Create") { model.createPlaylist() }
                                .disabled(model.newPlaylistName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || model.isStateBusy)
                            Button("Rename") { model.renameSelectedPlaylist() }
                                .disabled(model.selectedPlaylist == nil || model.newPlaylistName.trimmingCharacters(in: .whitespacesAndNewlines).isEmpty || model.isStateBusy)
                            Button("Add Track") { model.addSelectionToPlaylist() }
                                .disabled(model.selectedPlaylist == nil || model.selectedTrack == nil || model.isStateBusy)
                        }
                        HStack(spacing: 6) {
                            Button("Queue Playlist") { model.addSelectedPlaylistToQueue() }
                                .disabled(model.selectedPlaylist == nil || model.isStateBusy)
                            Button("Delete") { model.deleteSelectedPlaylist() }
                                .disabled(model.selectedPlaylist == nil || model.isStateBusy)
                        }
                    }
                }
                .groupBoxStyle(RustMusicGroupBoxStyle())
            }

            HStack(spacing: 12) {
                Button("Previous") { model.previous() }
                    .disabled((model.tracks.isEmpty && model.queueEntries.isEmpty) || model.isScanning || model.isStateBusy)
                Button("Next") { model.next() }
                    .disabled((model.tracks.isEmpty && model.queueEntries.isEmpty) || model.isScanning || model.isStateBusy)
                Button("Play") { model.play() }
                    .disabled(model.selectedTrack == nil || model.isPlaying || model.isScanning || model.isStateBusy)
                    .keyboardShortcut(.space, modifiers: [])
                Button("Stop") { model.stop() }
                    .disabled(!model.isPlaying)
                Spacer()
                Text(model.status)
                    .font(.callout)
                    .foregroundStyle(RustMusicPalette.muted)
                    .lineLimit(2)
            }

            Divider()
            Text("Scanning starts only when you load a folder. No autoplay or artwork; unsupported files are rejected.")
                .font(.callout)
                .foregroundStyle(RustMusicPalette.muted)
        }
        .padding(24)
        .foregroundStyle(RustMusicPalette.phosphor)
        .onChange(of: model.searchText) { _ in model.boundSearchText() }
        .onChange(of: model.selectedIndex) { _ in model.selectionChanged() }
        .onDisappear { model.cancelScan(); model.cancelStateOperation(); model.stop() }
    }
}

private struct RustMusicGroupBoxStyle: GroupBoxStyle {
    func makeBody(configuration: Configuration) -> some View {
        VStack(alignment: .leading, spacing: 8) {
            configuration.label
                .font(.caption.weight(.semibold))
                .foregroundStyle(RustMusicPalette.muted)
            configuration.content
        }
        .padding(14)
        .background(RustMusicPalette.surface, in: RoundedRectangle(cornerRadius: 12))
        .overlay {
            RoundedRectangle(cornerRadius: 12)
                .stroke(RustMusicPalette.line, lineWidth: 1)
        }
    }
}
