enum PlaybackUIState {
    static func flags(for state: UInt8) -> (playing: Bool, paused: Bool) {
        (playing: state == 2, paused: state == 6)
    }
}
