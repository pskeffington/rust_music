import Foundation

private func bigEndian(_ value: UInt16) -> Data {
    Data([UInt8(value >> 8), UInt8(value & 0xff)])
}

private func bigEndian(_ value: UInt32) -> Data {
    Data((0..<4).reversed().map { UInt8((value >> UInt32($0 * 8)) & 0xff) })
}

private func bigEndian(_ value: UInt64) -> Data {
    Data((0..<8).reversed().map { UInt8((value >> UInt64($0 * 8)) & 0xff) })
}

private func snapshotBody(
    state: UInt8 = 2,
    selected: UInt16? = 0,
    trackCount: UInt16 = 1,
    sampleRate: UInt32? = 44_100,
    channels: UInt8 = 1,
    bitsPerSample: UInt8 = 16,
    frames: UInt64 = 57_344,
    totalFrames: UInt64? = 441_000,
    audioClaimed: UInt8 = 1
) -> Data {
    var data = Data([state, selected == nil ? 0 : 1])
    if let selected { data.append(bigEndian(selected)) }
    data.append(bigEndian(trackCount))
    data.append(bigEndian(UInt16(0))) // page offset
    data.append(0) // empty page
    if let sampleRate {
        data.append(1)
        data.append(bigEndian(sampleRate))
        data.append(contentsOf: [channels, bitsPerSample])
    } else {
        data.append(0)
    }
    data.append(bigEndian(frames))
    if let totalFrames {
        data.append(1)
        data.append(bigEndian(totalFrames))
    } else {
        data.append(0)
    }
    data.append(audioClaimed)
    return data
}

private func decodeSnapshot(_ bytes: Data) throws -> PackagePlaybackSnapshot {
    var reader = PackageReader(bytes: Array(bytes))
    return try PackagePlaybackSnapshot.decode(from: &reader)
}

@main
struct PlaybackUIStateTests {
    static func main() {
        precondition(PlaybackUIState.flags(for: 2).playing)
        precondition(!PlaybackUIState.flags(for: 2).paused)
        precondition(!PlaybackUIState.flags(for: 6).playing)
        precondition(PlaybackUIState.flags(for: 6).paused)
        for terminalState: UInt8 in [3, 4, 5] {
            let flags = PlaybackUIState.flags(for: terminalState)
            precondition(!flags.playing && !flags.paused)
        }

        do {
            let known = try decodeSnapshot(snapshotBody())
            precondition(known.state == 2 && known.selected == 0)
            precondition(known.format == PlaybackPCMFormat(sampleRate: 44_100, channels: 1, bitsPerSample: 16))
            precondition(known.framesDelivered == 57_344 && known.totalFrames == 441_000)
            precondition(known.audioClaimed)
            precondition(known.elapsedText == "00:01" && known.durationText == "00:10")
            precondition(known.progressPerMille == 130)
            precondition(PackagePlaybackSnapshot.seekFrame(totalFrames: 441_000, perMille: 0) == 0)
            precondition(PackagePlaybackSnapshot.seekFrame(totalFrames: 441_000, perMille: 500) == 220_500)
            precondition(PackagePlaybackSnapshot.seekFrame(totalFrames: 441_000, perMille: 1000) == 441_000)
            precondition(PackagePlaybackSnapshot.seekFrame(totalFrames: 441_000, perMille: 1200) == 441_000)
            precondition(PackagePlaybackSnapshot.seekFrame(totalFrames: UInt64.max, perMille: 1000) == UInt64.max)
            precondition(PackagePlaybackSnapshot.seekFrame(totalFrames: 0, perMille: 500) == nil)

            let unknown = try decodeSnapshot(snapshotBody(state: 6, totalFrames: nil, audioClaimed: 0))
            precondition(unknown.state == 6 && !unknown.audioClaimed)
            precondition(unknown.elapsedText == "00:01" && unknown.durationText == nil)
            precondition(unknown.progressPerMille == nil)

            let overflow = try decodeSnapshot(snapshotBody(frames: UInt64.max, totalFrames: UInt64.max))
            precondition(overflow.progressPerMille == 1000)
            precondition(overflow.elapsedText != nil)

            let paused = try decodeSnapshot(snapshotBody(state: 6, audioClaimed: 0))
            let resumed = try decodeSnapshot(snapshotBody(state: 2, audioClaimed: 1))
            precondition(paused.framesDelivered == resumed.framesDelivered)
            precondition(!PlaybackUIState.shouldClearProgress(snapshot: paused, selectedIndex: 0))
            precondition(!PlaybackUIState.shouldClearProgress(snapshot: resumed, selectedIndex: 0))
            let finished = try decodeSnapshot(snapshotBody(state: 4, frames: 441_000))
            precondition(!PlaybackUIState.shouldClearProgress(snapshot: finished, selectedIndex: 0))
            precondition(PlaybackUIState.shouldClearProgress(snapshot: resumed, selectedIndex: 1))
            for terminalState: UInt8 in [0, 3, 5] {
                let terminal = try decodeSnapshot(snapshotBody(state: terminalState))
                precondition(PlaybackUIState.shouldClearProgress(snapshot: terminal, selectedIndex: 0))
            }

            let longDurationFrames = UInt64(44_100) * 3600 * 101
            let longDuration = try decodeSnapshot(snapshotBody(
                frames: longDurationFrames,
                totalFrames: longDurationFrames
            ))
            precondition(longDuration.elapsedText == "101:00:00")
            precondition(longDuration.durationText == "101:00:00")
        } catch {
            preconditionFailure("valid RBPK snapshot was rejected: \(error)")
        }

        precondition((try? decodeSnapshot(snapshotBody(sampleRate: 0))) == nil)
        precondition((try? decodeSnapshot(snapshotBody(channels: 0))) == nil)
        precondition((try? decodeSnapshot(snapshotBody(bitsPerSample: 12))) == nil)
        precondition((try? decodeSnapshot(snapshotBody(state: 7))) == nil)
        precondition((try? decodeSnapshot(snapshotBody(audioClaimed: 2))) == nil)
        precondition((try? decodeSnapshot(Data(snapshotBody().dropLast()))) == nil)
        var trailing = snapshotBody()
        trailing.append(0)
        precondition((try? decodeSnapshot(trailing)) == nil)
        print("PASS: RBPK v1 snapshot decoding, bounded progress, unknown duration, and state reset tests")
    }
}
