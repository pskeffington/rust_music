import AppKit

@main
private enum OperatorWindowLayoutTests {
    static func main() {
        let frames = OperatorWindowLayout.initialFrames(
            in: NSRect(x: 0, y: 0, width: 1440, height: 900)
        )
        precondition(frames.count == 4)
        precondition(frames[OperatorWindowLayout.player] == NSRect(x: 12, y: 604, width: 620, height: 284))
        precondition(frames[OperatorWindowLayout.playlist] == NSRect(x: 12, y: 308, width: 620, height: 284))
        precondition(frames[OperatorWindowLayout.equalizer] == NSRect(x: 12, y: 12, width: 620, height: 284))
        precondition(frames[OperatorWindowLayout.library] == NSRect(x: 644, y: 12, width: 784, height: 876))

        let rightPeer = NSRect(x: 0, y: 100, width: 100, height: 100)
        let nearlyDockedRight = NSRect(x: 111, y: 100, width: 100, height: 100)
        let snappedRight = OperatorWindowLayout.snappedFrame(nearlyDockedRight, to: [rightPeer])
        precondition(snappedRight.origin == NSPoint(x: 112, y: 100))

        let lowerPeer = NSRect(x: 100, y: 0, width: 100, height: 100)
        let nearlyDockedAbove = NSRect(x: 100, y: 111, width: 100, height: 100)
        let snappedAbove = OperatorWindowLayout.snappedFrame(nearlyDockedAbove, to: [lowerPeer])
        precondition(snappedAbove.origin == NSPoint(x: 100, y: 112))

        let distant = NSRect(x: 200, y: 100, width: 100, height: 100)
        precondition(OperatorWindowLayout.snappedFrame(distant, to: [rightPeer]) == distant)
        let unrelatedPeer = NSRect(x: 0, y: 0, width: 100, height: 100)
        precondition(OperatorWindowLayout.snappedFrame(nearlyDockedRight, to: [unrelatedPeer]) == nearlyDockedRight)
        precondition(OperatorWindowLayout.snappedFrame(nearlyDockedRight, to: [rightPeer], tolerance: 0) == nearlyDockedRight)
        print("PASS: operator window arrangement and magnetic docking geometry")
    }
}
