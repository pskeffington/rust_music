import Foundation

struct LibraryPanelLayout {
    static let compactContentHeight: CGFloat = 420

    let isCompact: Bool

    init(contentHeight: CGFloat) {
        isCompact = contentHeight < Self.compactContentHeight
    }

    var sectionSpacing: CGFloat { isCompact ? 4 : 12 }
    var contentSpacing: CGFloat { isCompact ? 4 : 8 }
    var contentPadding: CGFloat { isCompact ? 4 : 10 }
    var groupBoxPadding: CGFloat { isCompact ? 6 : 10 }
    var minimumResultsHeight: CGFloat { isCompact ? 40 : 96 }
}
