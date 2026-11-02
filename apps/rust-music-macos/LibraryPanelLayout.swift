import Foundation

struct LibraryPanelLayout {
    static let compactContentHeight: CGFloat = 420
    static let compactContentWidth: CGFloat = 644

    let isCompact: Bool
    let isCompactWidth: Bool
    let contentSize: CGSize

    init(contentSize: CGSize) {
        self.contentSize = contentSize
        isCompact = contentSize.height < Self.compactContentHeight
        isCompactWidth = contentSize.width < Self.compactContentWidth
    }

    var sectionSpacing: CGFloat { isCompact ? 4 : 12 }
    var contentSpacing: CGFloat { isCompact ? 4 : 8 }
    var contentPadding: CGFloat { isCompact ? 4 : 10 }
    var groupBoxPadding: CGFloat { isCompact ? 6 : 10 }
    var minimumResultsHeight: CGFloat { isCompact ? 40 : 96 }

    var toolbarItemSpacing: CGFloat { isCompactWidth ? 4 : 8 }
    var searchScopePickerWidth: CGFloat { isCompactWidth ? 92 : 86 }
    var sortPickerWidth: CGFloat { isCompactWidth ? 80 : 132 }
    var densityPickerWidth: CGFloat { isCompactWidth ? 92 : 160 }
    var compactControlFontSize: CGFloat { 9 }
    var countLabelFontSize: CGFloat { isCompactWidth ? 9 : 10 }
    var toolbarAvailableWidth: CGFloat {
        max(0, contentSize.width - 2 * contentPadding - 2 * groupBoxPadding)
    }

    func toolbarControlWidthBudget(countLabelWidth: CGFloat) -> CGFloat {
        searchScopePickerWidth
            + sortPickerWidth
            + densityPickerWidth
            + countLabelWidth
            + 4 * toolbarItemSpacing
    }
}
