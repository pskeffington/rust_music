import AppKit

enum OperatorWindowLayout {
    static let player = "RustMusic.Player"
    static let playlist = "RustMusic.Playlist"
    static let equalizer = "RustMusic.Equalizer"
    static let library = "RustMusic.Library"

    static let windowOrder = [player, playlist, equalizer, library]

    static let minimumFrames: [String: NSSize] = [
        player: NSSize(width: 520, height: 200),
        playlist: NSSize(width: 520, height: 200),
        equalizer: NSSize(width: 480, height: 200),
        library: NSSize(width: 520, height: 360)
    ]

    static func applyMinimumSize(for name: String, to window: NSWindow) {
        guard let minimumSize = minimumFrames[name] else { return }
        window.minSize = minimumSize
    }

    static func canTileAll(in visibleFrame: NSRect, gap: CGFloat = 12) -> Bool {
        let leftMinimum = max(
            minimumFrames[player]!.width,
            max(minimumFrames[playlist]!.width, minimumFrames[equalizer]!.width)
        )
        let leftHeight = minimumFrames[player]!.height
            + minimumFrames[playlist]!.height
            + minimumFrames[equalizer]!.height
            + gap * 2
        let tiledWidth = leftMinimum + minimumFrames[library]!.width + gap * 3
        let tiledHeight = max(leftHeight, minimumFrames[library]!.height) + gap * 2
        return visibleFrame.width >= tiledWidth && visibleFrame.height >= tiledHeight
    }

    static func initialFrames(
        in visibleFrame: NSRect,
        libraryWidth: CGFloat = 1000,
        gap: CGFloat = 12
    ) -> [String: NSRect] {
        guard canTileAll(in: visibleFrame, gap: gap) else {
            return cascadedFrames(in: visibleFrame, libraryWidth: libraryWidth, gap: gap)
        }
        let usableHeight = max(1, visibleFrame.height - gap * 2)
        let leftMinimum = max(
            minimumFrames[player]!.width,
            max(minimumFrames[playlist]!.width, minimumFrames[equalizer]!.width)
        )
        let leftWidth = min(620, max(leftMinimum, visibleFrame.width - gap * 3 - minimumFrames[library]!.width))
        let libraryX = visibleFrame.minX + gap + leftWidth + gap
        let availableLibraryWidth = max(1, visibleFrame.maxX - libraryX - gap)
        let libraryFrameWidth = min(max(minimumFrames[library]!.width, libraryWidth), availableLibraryWidth)
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

    static func fittedFrame(
        _ frame: NSRect,
        minimumSize: NSSize,
        in visibleFrame: NSRect,
        margin: CGFloat = 12
    ) -> NSRect {
        let availableWidth = max(1, visibleFrame.width - margin * 2)
        let availableHeight = max(1, visibleFrame.height - margin * 2)
        let width = min(max(frame.width, minimumSize.width), max(minimumSize.width, availableWidth))
        let height = min(max(frame.height, minimumSize.height), max(minimumSize.height, availableHeight))
        let minimumX = visibleFrame.minX + margin
        let maximumX = max(minimumX, visibleFrame.maxX - margin - width)
        let minimumY = visibleFrame.minY + margin
        let maximumY = max(minimumY, visibleFrame.maxY - margin - height)
        return NSRect(
            x: min(max(frame.minX, minimumX), maximumX),
            y: min(max(frame.minY, minimumY), maximumY),
            width: width,
            height: height
        )
    }

    private static func cascadedFrames(
        in visibleFrame: NSRect,
        libraryWidth: CGFloat,
        gap: CGFloat
    ) -> [String: NSRect] {
        let availableWidth = max(1, visibleFrame.width - gap * 2)
        let availableHeight = max(1, visibleFrame.height - gap * 2)
        let librarySize = minimumFrames[library]!
        let libraryFrame = NSRect(
            x: visibleFrame.minX + gap,
            y: visibleFrame.minY + gap,
            width: min(max(librarySize.width, libraryWidth), availableWidth),
            height: min(max(librarySize.height, 700), availableHeight)
        )
        return [
            player: NSRect(
                x: visibleFrame.minX + gap,
                y: visibleFrame.maxY - gap - minimumFrames[player]!.height,
                width: minimumFrames[player]!.width,
                height: minimumFrames[player]!.height
            ),
            playlist: NSRect(
                x: visibleFrame.minX + gap * 2,
                y: visibleFrame.maxY - gap * 2 - minimumFrames[playlist]!.height,
                width: minimumFrames[playlist]!.width,
                height: minimumFrames[playlist]!.height
            ),
            equalizer: NSRect(
                x: visibleFrame.minX + gap * 3,
                y: visibleFrame.maxY - gap * 3 - minimumFrames[equalizer]!.height,
                width: minimumFrames[equalizer]!.width,
                height: minimumFrames[equalizer]!.height
            ),
            library: libraryFrame
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
