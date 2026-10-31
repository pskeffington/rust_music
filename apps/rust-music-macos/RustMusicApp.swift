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

    private var scanProcess: Process?
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
        scanProcess.terminate()
        status = "Canceling library scan… the previous catalog is preserved."
    }

    private func loadLibrary(at root: URL) {
        guard !isScanning else { return }
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
            switch result {
            case .success(let tracks):
                self.libraryRoot = root
                self.tracks = tracks
                self.selectedIndex = tracks.first?.id
                self.status = "Loaded \(tracks.count) tracks. Playback is stopped."
            case .failure(let failure):
                self.status = failure.message
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

    func next() {
        moveSelection(forward: true)
    }

    func previous() {
        moveSelection(forward: false)
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
                .disabled(model.isScanning)
                .keyboardShortcut("o", modifiers: .command)
            }

            HStack(spacing: 10) {
                TextField("Search title, artist, album or path", text: $model.searchText)
                    .textFieldStyle(.roundedBorder)
                    .disabled(model.tracks.isEmpty)
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
                .frame(minHeight: 170)
            }
            .groupBoxStyle(RustMusicGroupBoxStyle())

            HStack(spacing: 12) {
                Button("Previous") { model.previous() }
                    .disabled(model.tracks.isEmpty || model.isScanning)
                Button("Next") { model.next() }
                    .disabled(model.tracks.isEmpty || model.isScanning)
                Button("Play") { model.play() }
                    .disabled(model.selectedTrack == nil || model.isPlaying || model.isScanning)
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
        .onDisappear { model.cancelScan(); model.stop() }
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
