import AppKit

@main
private enum LibraryPanelLayoutTests {
    static func main() {
        let compact = LibraryPanelLayout(contentSize: CGSize(width: 484, height: 320))
        precondition(compact.isCompact)
        precondition(compact.isCompactWidth)
        precondition(compact.sectionSpacing == 4)
        precondition(compact.contentSpacing == 4)
        precondition(compact.contentPadding == 4)
        precondition(compact.groupBoxPadding == 6)
        precondition(compact.minimumResultsHeight == 40)
        precondition(compact.toolbarAvailableWidth == 464)
        precondition(compact.searchScopePickerWidth == 92)
        precondition(compact.sortPickerWidth == 80)
        precondition(compact.densityPickerWidth == 92)
        precondition(compact.toolbarItemSpacing == 4)
        precondition(compact.countLabelFontSize == 9)

        let compactFont = NSFont.monospacedSystemFont(ofSize: compact.compactControlFontSize, weight: .medium)
        let standardFont = NSFont.monospacedSystemFont(ofSize: 10, weight: .medium)
        func measuredWidth(_ text: String, font: NSFont) -> CGFloat {
            (text as NSString).size(withAttributes: [.font: font]).width
        }
        let compactScanningCountWidth = measuredWidth("SCANNING · 20000 PRIOR ARTISTS", font: compactFont)
        let compactToolbarBudget = compact.toolbarControlWidthBudget(countLabelWidth: compactScanningCountWidth)
        precondition(compact.toolbarAvailableWidth - compactToolbarBudget >= 16)

        let labelValueGap: CGFloat = 6
        let chevronAllowance: CGFloat = 12
        let pickerBudget: (String, String) -> CGFloat = { label, value in
            measuredWidth(label, font: compactFont)
                + labelValueGap
                + measuredWidth(value, font: compactFont)
                + chevronAllowance
        }
        precondition(pickerBudget("IN", "ALL FIELDS") <= compact.searchScopePickerWidth)
        precondition(pickerBudget("SORT", "ARTIST") <= compact.sortPickerWidth)
        precondition(pickerBudget("ROWS", "STANDARD") <= compact.densityPickerWidth)

        let standard = LibraryPanelLayout(contentSize: CGSize(width: 964, height: 660))
        precondition(!standard.isCompact)
        precondition(!standard.isCompactWidth)
        precondition(standard.sectionSpacing == 12)
        precondition(standard.contentSpacing == 8)
        precondition(standard.contentPadding == 10)
        precondition(standard.groupBoxPadding == 10)
        precondition(standard.minimumResultsHeight == 96)
        precondition(standard.toolbarAvailableWidth == 924)
        precondition(standard.searchScopePickerWidth == 86)
        precondition(standard.sortPickerWidth == 132)
        precondition(standard.densityPickerWidth == 160)
        precondition(standard.toolbarItemSpacing == 8)
        precondition(standard.countLabelFontSize == 10)
        let standardScanningCountWidth = measuredWidth("SCANNING · 20000 PRIOR ARTISTS", font: standardFont)
        let standardBudget = standard.toolbarControlWidthBudget(countLabelWidth: standardScanningCountWidth)
        precondition(standardBudget < standard.toolbarAvailableWidth)
        let justBelowStandardWidth = LibraryPanelLayout(contentSize: CGSize(width: 643, height: 700))
        precondition(justBelowStandardWidth.isCompactWidth)
        precondition(
            justBelowStandardWidth.toolbarControlWidthBudget(countLabelWidth: compactScanningCountWidth)
                <= justBelowStandardWidth.toolbarAvailableWidth
        )
        let firstStandardWidth = LibraryPanelLayout(contentSize: CGSize(width: 644, height: 700))
        precondition(!firstStandardWidth.isCompactWidth)
        precondition(standardBudget <= firstStandardWidth.toolbarAvailableWidth)
    }
}
