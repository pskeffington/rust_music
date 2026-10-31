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
    @Published var selectedFile: URL?
    @Published var audioInfo = "Choose one local audio file to inspect it."
    @Published var status = "Ready — playback is stopped."
    @Published var isPlaying = false

    private var playProcess: Process?
    private var parentGuardPipe: Pipe?
    private var escalationScheduledFor: Int32?
    private var stopRequested = false

    private var decoderURL: URL {
        Bundle.main.bundleURL.appendingPathComponent("Contents/MacOS/rust-music-cli")
    }

    func chooseFile() {
        let picker = NSOpenPanel()
        picker.title = "Choose Lossless Audio"
        picker.prompt = "Inspect"
        picker.canChooseFiles = true
        picker.canChooseDirectories = false
        picker.allowsMultipleSelection = false
        picker.allowedContentTypes = [.audio]
        guard picker.runModal() == .OK, let url = picker.url else { return }
        selectedFile = url
        inspect(url)
    }

    func inspect(_ url: URL) {
        status = "Validating with the Rust Music decoder…"
        Task.detached(priority: .userInitiated) { [decoderURL] in
            let result = Self.run(decoder: decoderURL, arguments: ["inspect", url.path])
            await MainActor.run {
                guard self.selectedFile == url else { return }
                switch result {
                case .success(let value):
                    self.audioInfo = value.trimmingCharacters(in: .whitespacesAndNewlines)
                    self.status = "Ready — no audio has started."
                case .failure(let failure):
                    self.audioInfo = "This file is not admitted for playback."
                    self.status = failure.message
                }
            }
        }
    }

    func play() {
        guard let selectedFile, playProcess == nil else { return }
        let process = Process()
        process.executableURL = decoderURL
        process.arguments = ["play-managed", selectedFile.path]
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

    nonisolated private static func run(decoder: URL, arguments: [String]) -> Result<String, RustMusicFailure> {
        let process = Process()
        process.executableURL = decoder
        process.arguments = arguments
        let output = Pipe()
        let errors = Pipe()
        process.standardOutput = output
        process.standardError = errors
        do {
            try process.run()
            let outputData = output.fileHandleForReading.readDataToEndOfFile()
            let errorData = errors.fileHandleForReading.readDataToEndOfFile()
            process.waitUntilExit()
            if process.terminationStatus == 0 {
                return .success(String(data: outputData, encoding: .utf8) ?? "")
            }
            let message = String(data: errorData, encoding: .utf8)?
                .trimmingCharacters(in: .whitespacesAndNewlines)
            return .failure(RustMusicFailure(
                message: message?.isEmpty == false ? message ?? "" : "Rust Music rejected the file."
            ))
        } catch {
            return .failure(RustMusicFailure(
                message: "Could not run Rust Music: \(error.localizedDescription)"
            ))
        }
    }
}

struct PlayerView: View {
    @StateObject private var model = PlayerModel()

    var body: some View {
        VStack(alignment: .leading, spacing: 22) {
            HStack(alignment: .top, spacing: 14) {
                Image(systemName: "waveform")
                    .font(.system(size: 28, weight: .medium))
                    .foregroundStyle(RustMusicPalette.phosphor)
                    .frame(width: 48, height: 48)
                    .background(RustMusicPalette.surface, in: RoundedRectangle(cornerRadius: 12))
                VStack(alignment: .leading, spacing: 4) {
                    Text("Rust Music")
                        .font(.largeTitle.weight(.semibold))
                    Text("Private, offline playback for lossless audio")
                        .foregroundStyle(RustMusicPalette.muted)
                }
                Spacer()
                Button("Choose File…", systemImage: "folder") {
                    model.chooseFile()
                }
                .keyboardShortcut("o", modifiers: .command)
            }

            GroupBox("Selected Track") {
                VStack(alignment: .leading, spacing: 10) {
                    Text(model.selectedFile?.lastPathComponent ?? "No file selected")
                        .font(.headline)
                        .lineLimit(2)
                        .textSelection(.enabled)
                    Text(model.audioInfo)
                        .font(.system(.body, design: .monospaced))
                        .foregroundStyle(RustMusicPalette.phosphor)
                        .textSelection(.enabled)
                        .frame(maxWidth: .infinity, minHeight: 48, alignment: .leading)
                }
                .padding(.vertical, 4)
                .padding(.horizontal, 2)
            }
            .groupBoxStyle(RustMusicGroupBoxStyle())

            HStack(spacing: 12) {
                Button("Play", systemImage: "play.fill") { model.play() }
                    .disabled(model.selectedFile == nil || model.isPlaying)
                    .keyboardShortcut(.space, modifiers: [])
                Button("Stop", systemImage: "stop.fill") { model.stop() }
                    .disabled(!model.isPlaying)
                Spacer()
                Label(model.status, systemImage: model.isPlaying ? "speaker.wave.2" : "checkmark.circle")
                    .font(.callout)
                    .foregroundStyle(RustMusicPalette.muted)
                    .lineLimit(2)
            }

            Divider()
            Label("No background scan or autoplay. Lossy and unsupported files are rejected by Rust Music.",
                  systemImage: "lock.shield")
                .font(.callout)
                .foregroundStyle(RustMusicPalette.muted)
        }
        .padding(24)
        .foregroundStyle(RustMusicPalette.phosphor)
        .onDisappear { model.stop() }
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
