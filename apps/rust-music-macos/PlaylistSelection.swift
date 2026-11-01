import Foundation

enum PlaylistSelection {
    /// Returns the selected row indexes after moving the selected entries one
    /// adjacent position while preserving their relative order.
    static func indices(afterMoving selected: Set<Int>, in count: Int, by offset: Int) -> Set<Int> {
        guard count > 0, offset == -1 || offset == 1,
              !selected.isEmpty,
              selected.allSatisfy({ $0 >= 0 && $0 < count }) else { return [] }

        var order = Array(0..<count)
        let sources = offset < 0 ? selected.sorted() : selected.sorted(by: >)
        for source in sources {
            let target = source + offset
            if (0..<count).contains(target) {
                order.swapAt(source, target)
            }
        }

        return Set(order.enumerated().compactMap { index, original in
            selected.contains(original) ? index : nil
        })
    }
}
