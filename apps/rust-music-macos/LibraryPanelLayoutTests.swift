import Foundation

@main
private enum LibraryPanelLayoutTests {
    static func main() {
        let compact = LibraryPanelLayout(contentHeight: 360)
        precondition(compact.isCompact)
        precondition(compact.sectionSpacing == 4)
        precondition(compact.contentSpacing == 4)
        precondition(compact.contentPadding == 4)
        precondition(compact.groupBoxPadding == 6)
        precondition(compact.minimumResultsHeight == 40)

        let standard = LibraryPanelLayout(contentHeight: 420)
        precondition(!standard.isCompact)
        precondition(standard.sectionSpacing == 12)
        precondition(standard.contentSpacing == 8)
        precondition(standard.contentPadding == 10)
        precondition(standard.groupBoxPadding == 10)
        precondition(standard.minimumResultsHeight == 96)
    }
}
