import Foundation

struct RustMusicFailure: LocalizedError {
    let message: String
    var errorDescription: String? { message }
}

struct PackageReader {
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
        guard count <= max else {
            throw RustMusicFailure(message: "Rust Music returned oversized package text.")
        }
        guard let value = String(bytes: try take(count), encoding: .utf8),
              !value.unicodeScalars.contains(where: { CharacterSet.controlCharacters.contains($0) }) else {
            throw RustMusicFailure(message: "Rust Music returned invalid package text.")
        }
        return value
    }
}

struct PlaybackPCMFormat: Equatable {
    let sampleRate: UInt32
    let channels: UInt8
    let bitsPerSample: UInt8
}

struct LibraryStateRevision {
    private(set) var value: UInt64 = 0

    mutating func advance() {
        value &+= 1
    }

    func accepts(_ operationRevision: UInt64) -> Bool {
        operationRevision == value
    }
}

struct DeferredStateReload {
    private(set) var isPending = false

    mutating func request(isStateBusy: Bool, isTransportBusy: Bool) -> Bool {
        isPending = true
        return takeIfAvailable(isStateBusy: isStateBusy, isTransportBusy: isTransportBusy)
    }

    mutating func takeIfAvailable(isStateBusy: Bool, isTransportBusy: Bool) -> Bool {
        guard isPending, !isStateBusy, !isTransportBusy else { return false }
        isPending = false
        return true
    }
}

struct PackagePlaybackSnapshot: Equatable {
    let state: UInt8
    let selected: UInt16?
    let trackCount: UInt16
    let format: PlaybackPCMFormat?
    let framesDelivered: UInt64
    let totalFrames: UInt64?
    let audioClaimed: Bool

    /// Decode the RBPK v1 View body after the reply tag. The snapshot reports
    /// frames accepted by the output queue; it is not a hardware playback clock.
    static func decode(from reader: inout PackageReader) throws -> Self {
        let state = try reader.byte()
        guard state <= 6 else {
            throw RustMusicFailure(message: "Rust Music returned an unknown playback state.")
        }
        let selectedPresent = try reader.byte()
        guard selectedPresent <= 1 else {
            throw RustMusicFailure(message: "Rust Music returned an invalid selection flag.")
        }
        let selected = selectedPresent == 1 ? try reader.u16() : nil
        let trackCount = try reader.u16()
        _ = try reader.u16() // page offset
        let pageCount = Int(try reader.byte())
        guard pageCount <= 32 else {
            throw RustMusicFailure(message: "Rust Music returned too many package tracks.")
        }
        for _ in 0..<pageCount {
            let index = try reader.u16()
            _ = try reader.text(max: 128)
            let container = try reader.byte()
            guard container <= 4, index < trackCount else {
                throw RustMusicFailure(message: "Rust Music returned an invalid package track.")
            }
        }
        if let selected, selected >= trackCount {
            throw RustMusicFailure(message: "Rust Music returned a selection outside the library.")
        }

        let formatPresent = try reader.byte()
        guard formatPresent <= 1 else {
            throw RustMusicFailure(message: "Rust Music returned an invalid PCM format flag.")
        }
        let format: PlaybackPCMFormat?
        if formatPresent == 1 {
            let sampleRate = try reader.u32()
            let channels = try reader.byte()
            let bitsPerSample = try reader.byte()
            guard (1...1_048_575).contains(sampleRate), (1...8).contains(channels),
                  [8, 16, 24, 32].contains(bitsPerSample) else {
                throw RustMusicFailure(message: "Rust Music returned an invalid PCM format.")
            }
            format = PlaybackPCMFormat(
                sampleRate: sampleRate,
                channels: channels,
                bitsPerSample: bitsPerSample
            )
        } else {
            format = nil
        }

        let framesDelivered = try reader.u64()
        let totalPresent = try reader.byte()
        guard totalPresent <= 1 else {
            throw RustMusicFailure(message: "Rust Music returned an invalid frame-count flag.")
        }
        let totalFrames = totalPresent == 1 ? try reader.u64() : nil
        let audioClaimedValue = try reader.byte()
        guard audioClaimedValue <= 1 else {
            throw RustMusicFailure(message: "Rust Music returned an invalid audio-claim flag.")
        }
        guard reader.offset == reader.bytes.count else {
            throw RustMusicFailure(message: "Rust Music package snapshot has trailing data.")
        }
        return Self(
            state: state,
            selected: selected,
            trackCount: trackCount,
            format: format,
            framesDelivered: framesDelivered,
            totalFrames: totalFrames,
            audioClaimed: audioClaimedValue == 1
        )
    }

    var progressPerMille: UInt16? {
        guard let totalFrames, totalFrames > 0 else { return nil }
        let frames = min(framesDelivered, totalFrames)
        guard frames < totalFrames else { return 1000 }
        let product = frames.multipliedFullWidth(by: 1000)
        // frames < totalFrames guarantees the high half is smaller than the
        // divisor, so Swift's full-width division cannot overflow its quotient.
        return UInt16(totalFrames.dividingFullWidth(product).quotient)
    }

    static func seekFrame(totalFrames: UInt64, perMille: Int) -> UInt64? {
        guard totalFrames > 0 else { return nil }
        let bounded = UInt64(min(1000, max(0, perMille)))
        // Split before multiplying so even UInt64.max frame counts cannot
        // overflow while converting the scrubber's 0...1000 range.
        return (totalFrames / 1000) * bounded
            + ((totalFrames % 1000) * bounded) / 1000
    }

    var elapsedText: String? {
        guard let format, format.sampleRate > 0 else { return nil }
        let frames = totalFrames.map { min(framesDelivered, $0) } ?? framesDelivered
        let seconds = frames / UInt64(format.sampleRate)
        return Self.timeText(seconds)
    }

    var durationText: String? {
        guard let format, format.sampleRate > 0, let totalFrames else { return nil }
        return Self.timeText(totalFrames / UInt64(format.sampleRate))
    }

    var outputSummary: String {
        guard let format else {
            return "PCM FORMAT UNKNOWN · \(audioClaimed ? "OUTPUT CLAIMED" : "NO OUTPUT CLAIM")"
        }
        let rate = String(format: "%.1f kHz", Double(format.sampleRate) / 1000)
        let channels = format.channels == 1 ? "MONO" : "\(format.channels) CH"
        let frames = String(format: "%llu", framesDelivered)
        return "\(rate) · \(format.bitsPerSample)-BIT · \(channels) · \(frames) FRAMES SUBMITTED · \(audioClaimed ? "OUTPUT CLAIMED" : "NO OUTPUT CLAIM")"
    }

    var progressAccessibilityValue: String {
        let elapsed = elapsedText ?? "unknown elapsed time"
        let duration = durationText ?? "unknown duration"
        return "\(elapsed) of \(duration); \(framesDelivered) frames submitted to the output queue. Audible output is not confirmed."
    }

    private static func timeText(_ seconds: UInt64) -> String {
        let hours = seconds / 3600
        let minutes = (seconds % 3600) / 60
        let remainder = seconds % 60
        return hours > 0
            ? String(format: "%llu:%02llu:%02llu", hours, minutes, remainder)
            : String(format: "%02llu:%02llu", minutes, remainder)
    }
}

enum PlaybackUIState {
    static func flags(for state: UInt8) -> (playing: Bool, paused: Bool) {
        (playing: state == 2, paused: state == 6)
    }

    static func shouldClearProgress(snapshot: PackagePlaybackSnapshot, selectedIndex: Int?) -> Bool {
        guard snapshot.state != 0, snapshot.state != 3, snapshot.state != 5,
              let selectedIndex, let selected = UInt16(exactly: selectedIndex) else {
            return true
        }
        return snapshot.selected != selected
    }
}
