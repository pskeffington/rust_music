import Foundation

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
        print("PASS: playback UI state clears active controls for stopped, finished, and failed snapshots")
    }
}
