import Foundation

@main
struct PlaylistSelectionTests {
    static func main() {
        precondition(PlaylistSelection.indices(afterMoving: [0, 1], in: 3, by: -1) == [0, 1])
        precondition(PlaylistSelection.indices(afterMoving: [1, 2], in: 3, by: 1) == [1, 2])
        precondition(PlaylistSelection.indices(afterMoving: [1, 3], in: 5, by: -1) == [0, 2])
        precondition(PlaylistSelection.indices(afterMoving: [0, 2], in: 4, by: 1) == [1, 3])
        precondition(PlaylistSelection.indices(afterMoving: [], in: 4, by: 1).isEmpty)
        precondition(PlaylistSelection.indices(afterMoving: [4], in: 4, by: -1).isEmpty)
        print("PASS: playlist multi-selection remains stable across edge and separated moves")
    }
}
