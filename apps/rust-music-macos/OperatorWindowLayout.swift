import AppKit

enum OperatorWindowLayout {
    static let player = "RustMusic.Player"
    static let playlist = "RustMusic.Playlist"
    static let equalizer = "RustMusic.Equalizer"
    static let library = "RustMusic.Library"

    static let windowOrder = [player, playlist, equalizer, library]

    static func initialFrames(
        in visibleFrame: NSRect,
        libraryWidth: CGFloat = 1000,
        gap: CGFloat = 12
    ) -> [String: NSRect] {
        let usableHeight = max(1, visibleFrame.height - gap * 2)
        let leftWidth = min(620, max(200, visibleFrame.width - gap * 3 - 680))
        let libraryX = visibleFrame.minX + gap + leftWidth + gap
        let availableLibraryWidth = max(1, visibleFrame.maxX - libraryX - gap)
        let libraryFrameWidth = min(max(1, libraryWidth), availableLibraryWidth)
        let slotHeight = max(1, (usableHeight - gap * 2) / 3)
        let leftX = visibleFrame.minX + gap
        let topY = visibleFrame.maxY - gap - slotHeight

        return [
            player: NSRect(x: leftX, y: topY, width: leftWidth, height: slotHeight),
            playlist: NSRect(
                x: leftX,
                y: topY - slotHeight - gap,
                width: leftWidth,
                height: slotHeight
            ),
            equalizer: NSRect(
                x: leftX,
                y: visibleFrame.minY + gap,
                width: leftWidth,
                height: slotHeight
            ),
            library: NSRect(
                x: libraryX,
                y: visibleFrame.minY + gap,
                width: libraryFrameWidth,
                height: usableHeight
            )
        ]
    }

    static func snappedFrame(
        _ frame: NSRect,
        to peers: [NSRect],
        gap: CGFloat = 12,
        tolerance: CGFloat = 16,
        minimumOverlap: CGFloat = 40
    ) -> NSRect {
        guard tolerance > 0 else { return frame }
        var result = frame
        let deltaX = closestDelta(
            among: peers.flatMap { peer -> [CGFloat] in
                guard verticalOverlap(frame, peer) >= minimumOverlap else { return [] }
                return [
                    peer.minX - gap - frame.maxX,
                    peer.maxX + gap - frame.minX,
                    peer.minX - frame.minX,
                    peer.maxX - frame.maxX
                ]
            },
            tolerance: tolerance
        )
        let deltaY = closestDelta(
            among: peers.flatMap { peer -> [CGFloat] in
                guard horizontalOverlap(frame, peer) >= minimumOverlap else { return [] }
                return [
                    peer.minY - gap - frame.maxY,
                    peer.maxY + gap - frame.minY,
                    peer.minY - frame.minY,
                    peer.maxY - frame.maxY
                ]
            },
            tolerance: tolerance
        )
        result.origin.x += deltaX
        result.origin.y += deltaY
        return result
    }

    private static func closestDelta(among values: [CGFloat], tolerance: CGFloat) -> CGFloat {
        values
            .filter { abs($0) <= tolerance }
            .min { abs($0) < abs($1) } ?? 0
    }

    private static func verticalOverlap(_ lhs: NSRect, _ rhs: NSRect) -> CGFloat {
        max(0, min(lhs.maxY, rhs.maxY) - max(lhs.minY, rhs.minY))
    }

    private static func horizontalOverlap(_ lhs: NSRect, _ rhs: NSRect) -> CGFloat {
        max(0, min(lhs.maxX, rhs.maxX) - max(lhs.minX, rhs.minX))
    }
}
