import Foundation

private enum CatalogTextNormalizer {
    // Search keys are cached at app-lifetime locale. A system locale change
    // requires reloading the library so each key is rebuilt consistently.
    static let locale = Locale.current

    static func fold(_ text: String) -> String {
        text.decomposedStringWithCanonicalMapping
            .folding(options: [.caseInsensitive], locale: locale)
    }
}

struct CatalogTrack: Identifiable, Equatable {
    let id: Int
    let container: String
    let title: String
    let artist: String
    let album: String
    let relativePath: String
    let searchableBytes: Data
    let titleSortKey: String
    let artistSortKey: String
    let albumSortKey: String

    init(
        id: Int,
        container: String,
        title: String,
        artist: String,
        album: String,
        relativePath: String
    ) {
        self.id = id
        self.container = container
        self.title = title
        self.artist = artist
        self.album = album
        self.relativePath = relativePath
        titleSortKey = CatalogTextNormalizer.fold(title)
        artistSortKey = CatalogTextNormalizer.fold(artist)
        albumSortKey = CatalogTextNormalizer.fold(album)
        let searchableText = [title, artist, album, relativePath]
            .map(CatalogTextNormalizer.fold)
            .joined(separator: "\u{0}")
        searchableBytes = Data(searchableText.utf8)
    }
}

enum CatalogSearch {
    static let maximumPageSize = 200

    enum BrowseMode: String, CaseIterable, Equatable, Identifiable {
        case tracks
        case artists
        case albums

        var id: String { rawValue }
        var label: String { rawValue.uppercased() }
        var groupSearchScope: SearchScope? {
            switch self {
            case .tracks: return nil
            case .artists: return .artist
            case .albums: return .album
            }
        }
    }

    struct BrowseGroup: Identifiable, Equatable {
        let id: String
        let name: String
        let trackCount: Int
        let searchKey: String
        let detail: String?
        let artistFilterKey: String?
        let albumFilterKey: String?
    }

    struct BrowsePage {
        let groups: [BrowseGroup]
        let totalMatches: Int
        let offset: Int
        let pageSize: Int
    }

    struct Page {
        let tracks: [CatalogTrack]
        let totalMatches: Int
        let offset: Int
        let pageSize: Int
    }

    enum SortOrder: String, CaseIterable, Identifiable {
        case title
        case artist
        case album

        var id: String { rawValue }
        var label: String { rawValue.uppercased() }

        init?(rawValue: String) {
            switch rawValue {
            case "title": self = .title
            case "artist": self = .artist
            case "album": self = .album
            default: return nil
            }
        }
    }

    enum SearchScope: String, CaseIterable, Equatable, Identifiable {
        case all
        case title
        case artist
        case album

        var id: String { rawValue }
        var label: String { rawValue.uppercased() }

        init?(rawValue: String) {
            switch rawValue {
            case "all": self = .all
            case "title": self = .title
            case "artist": self = .artist
            case "album": self = .album
            default: return nil
            }
        }
    }

    static func visibleTracks(
        in tracks: [CatalogTrack],
        matching searchText: String,
        sortedBy order: SortOrder = .title,
        in scope: SearchScope = .all
    ) -> [CatalogTrack] {
        CatalogSearchIndex(tracks: tracks).visibleTracks(matching: searchText, sortedBy: order, in: scope)
    }

    static func matches(
        _ track: CatalogTrack,
        matching searchText: String,
        in scope: SearchScope = .all
    ) -> Bool {
        switch scope {
        case .all:
            guard let queryBytes = queryBytes(for: searchText) else { return true }
            return track.searchableBytes.range(of: queryBytes) != nil
        case .title, .artist, .album:
            guard let query = queryText(for: searchText) else { return true }
            let key: String
            switch scope {
            case .title: key = track.titleSortKey
            case .artist: key = track.artistSortKey
            case .album: key = track.albumSortKey
            case .all: return true
            }
            return key.range(of: query) != nil
        }
    }

    static func isActionableSelectedTrack(
        _ track: CatalogTrack?,
        selectedTrackIDs: Set<Int>,
        matching searchText: String,
        in scope: SearchScope = .all
    ) -> Bool {
        guard let track, selectedTrackIDs.contains(track.id) else { return false }
        return matches(track, matching: searchText, in: scope)
    }

    static func mergingPageSelection(
        _ selectedTrackIDs: Set<Int>,
        visibleTrackIDs: Set<Int>,
        proposedVisibleSelection: Set<Int>
    ) -> Set<Int> {
        selectedTrackIDs.subtracting(visibleTrackIDs)
            .union(proposedVisibleSelection.intersection(visibleTrackIDs))
    }

    static func selectedTrackCountOutsideView(
        _ selectedTrackIDs: Set<Int>,
        visibleTrackIDs: Set<Int>
    ) -> Int {
        selectedTrackIDs.subtracting(visibleTrackIDs).count
    }

    static func queryBytes(for searchText: String) -> Data? {
        guard let query = queryText(for: searchText) else { return nil }
        return Data(query.utf8)
    }

    static func queryText(for searchText: String) -> String? {
        let query = boundedQuery(searchText).trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return nil }
        return CatalogTextNormalizer.fold(query)
    }

    static func boundedQuery(_ text: String) -> String {
        var bounded = ""
        var byteCount = 0
        for scalar in text.unicodeScalars {
            let value = scalar.value
            if value < 0x20 || (0x7f...0x9f).contains(value) { continue }
            let scalarBytes = String(scalar).utf8.count
            guard byteCount + scalarBytes <= 512 else { break }
            bounded.unicodeScalars.append(scalar)
            byteCount += scalarBytes
        }
        return bounded
    }
}

enum LibraryRowDensity: String, CaseIterable, Identifiable {
    case compact
    case standard

    var id: String { rawValue }
    var label: String { rawValue.uppercased() }
    var titleFontSize: CGFloat { self == .compact ? 11 : 12 }
    var metadataFontSize: CGFloat { self == .compact ? 9 : 10 }
    var textSpacing: CGFloat { self == .compact ? 1 : 2 }
    var verticalPadding: CGFloat { self == .compact ? 0 : 2 }
}

struct CatalogPageNavigation: Equatable {
    let totalItems: Int
    let offset: Int
    let visibleItemCount: Int
    let pageSize: Int

    init(totalItems: Int, offset: Int, visibleItemCount: Int, pageSize: Int = CatalogSearch.maximumPageSize) {
        self.totalItems = max(totalItems, 0)
        self.pageSize = max(pageSize, 1)
        self.offset = self.totalItems == 0 ? 0 : min(max(offset, 0), self.totalItems - 1)
        self.visibleItemCount = min(max(visibleItemCount, 0), max(self.totalItems - self.offset, 0))
    }

    var pageCount: Int {
        totalItems == 0 ? 1 : 1 + (totalItems - 1) / pageSize
    }

    var currentPage: Int {
        min(offset / pageSize + 1, pageCount)
    }

    var firstItem: Int {
        visibleItemCount == 0 ? 0 : offset + 1
    }

    var lastItem: Int {
        offset + visibleItemCount
    }

    func adjacentOffset(forward: Bool) -> Int? {
        let pageIndex = offset / pageSize
        if forward {
            guard pageIndex < pageCount - 1 else { return nil }
            return (pageIndex + 1) * pageSize
        }
        guard pageIndex > 0 else { return nil }
        return (pageIndex - 1) * pageSize
    }
}

/// Catalog-lifetime sort indexes. Queries filter an already ordered index, so
/// typing does not sort the full matching catalog again. Lazily builds each
/// requested order and keeps deterministic ID tie-breaking.
final class CatalogSearchIndex {
    private let tracks: [CatalogTrack]
    private let trackIDsMatchSourceOffsets: Bool
    private var titleOrder: [Int]?
    private var artistOrder: [Int]?
    private var albumOrder: [Int]?
    private var artistGroups: [CatalogSearch.BrowseGroup]?
    private var albumGroups: [CatalogSearch.BrowseGroup]?
    private var foldedArtistKeys: [String]?

    init(tracks: [CatalogTrack]) {
        self.tracks = tracks
        trackIDsMatchSourceOffsets = tracks.enumerated().allSatisfy { $0.offset == $0.element.id }
    }

    func visibleTracks(
        matching searchText: String,
        sortedBy order: CatalogSearch.SortOrder = .title,
        in scope: CatalogSearch.SearchScope = .all
    ) -> [CatalogTrack] {
        guard scope == .all else {
            return tracks.filter { CatalogSearch.matches($0, matching: searchText, in: scope) }
                .sorted { comesBefore($0, $1, by: order) }
        }
        let queryBytes = CatalogSearch.queryBytes(for: searchText)
        guard let queryBytes else {
            return sortedIndices(for: order).map { tracks[$0] }
        }

        // For ordinary, selective queries, sorting a small match list is
        // faster than walking the entire sorted index. Broad queries switch
        // to a compact bitmap so result ordering still comes from the cache.
        let smallMatchLimit = 1_024
        var matchingTracks = tracks.filter {
            $0.searchableBytes.range(of: queryBytes) != nil
        }
        if matchingTracks.count <= smallMatchLimit || !trackIDsMatchSourceOffsets {
            matchingTracks.sort { comesBefore($0, $1, by: order) }
            return matchingTracks
        }

        let wordCount = tracks.count / 64 + (tracks.count.isMultiple(of: 64) ? 0 : 1)
        var matchingBits = [UInt64](repeating: 0, count: wordCount)
        for track in matchingTracks {
            matchingBits[track.id >> 6] |= UInt64(1) << UInt64(track.id & 63)
        }
        let matchingCount = matchingTracks.count
        matchingTracks.removeAll(keepingCapacity: false)

        let indices = sortedIndices(for: order)
        var result: [CatalogTrack] = []
        result.reserveCapacity(matchingCount)
        for index in indices where matchingBits[index >> 6] & (UInt64(1) << UInt64(index & 63)) != 0 {
            result.append(tracks[index])
        }
        return result
    }

    /// Materializes at most `limit` rows while counting the full result set.
    /// Filtering and sorting stay on the catalog-view queue; SwiftUI receives
    /// only a bounded page, even when the query matches the whole catalog.
    func page(
        matching searchText: String,
        sortedBy order: CatalogSearch.SortOrder = .title,
        in scope: CatalogSearch.SearchScope = .all,
        withinArtistKey: String? = nil,
        withinAlbumKey: String? = nil,
        offset requestedOffset: Int,
        limit requestedLimit: Int = CatalogSearch.maximumPageSize
    ) -> CatalogSearch.Page {
        let limit = min(max(requestedLimit, 1), CatalogSearch.maximumPageSize)
        let offset = max(requestedOffset, 0)
        let queryBytes = scope == .all ? CatalogSearch.queryBytes(for: searchText) : nil
        let scopedQueryIsEmpty = scope == .all || CatalogSearch.queryText(for: searchText) == nil
        let indices = sortedIndices(for: order)
        let artistKeys = withinArtistKey == nil ? nil : cachedArtistBrowseKeys()
        func matchesBrowseFilters(at index: Int) -> Bool {
            if let withinArtistKey, artistKeys?[index] != withinArtistKey { return false }
            guard let withinAlbumKey else { return true }
            return CatalogTextNormalizer.fold(Self.albumName(tracks[index].album)) == withinAlbumKey
        }
        guard let queryBytes else {
            if !scopedQueryIsEmpty || withinArtistKey != nil {
                var totalMatches = 0
                var pageTracks: [CatalogTrack] = []
                pageTracks.reserveCapacity(limit)
                for index in indices where CatalogSearch.matches(tracks[index], matching: searchText, in: scope)
                    && matchesBrowseFilters(at: index) {
                    if totalMatches >= offset, pageTracks.count < limit { pageTracks.append(tracks[index]) }
                    totalMatches += 1
                }
                return CatalogSearch.Page(
                    tracks: pageTracks,
                    totalMatches: totalMatches,
                    offset: min(offset, totalMatches),
                    pageSize: limit
                )
            }
            let start = min(offset, tracks.count)
            let end = min(start + limit, tracks.count)
            return CatalogSearch.Page(
                tracks: indices[start..<end].map { tracks[$0] },
                totalMatches: tracks.count,
                offset: start,
                pageSize: limit
            )
        }

        var totalMatches = 0
        var pageTracks: [CatalogTrack] = []
        pageTracks.reserveCapacity(limit)
        for index in indices where tracks[index].searchableBytes.range(of: queryBytes) != nil
            && matchesBrowseFilters(at: index) {
            if totalMatches >= offset, pageTracks.count < limit {
                pageTracks.append(tracks[index])
            }
            totalMatches += 1
        }
        return CatalogSearch.Page(
            tracks: pageTracks,
            totalMatches: totalMatches,
            offset: min(offset, totalMatches),
            pageSize: limit
        )
    }

    /// Builds a sorted, deduplicated Artist or Album index once, then returns
    /// a bounded page. Missing tags appear under a clear fallback label.
    func browsePage(
        for mode: CatalogSearch.BrowseMode,
        matching searchText: String,
        offset requestedOffset: Int,
        limit requestedLimit: Int = CatalogSearch.maximumPageSize
    ) -> CatalogSearch.BrowsePage {
        guard mode != .tracks else {
            return CatalogSearch.BrowsePage(groups: [], totalMatches: 0, offset: 0, pageSize: 0)
        }
        let limit = min(max(requestedLimit, 1), CatalogSearch.maximumPageSize)
        let offset = max(requestedOffset, 0)
        let groups = browseGroups(for: mode)
        guard let query = CatalogSearch.queryText(for: searchText) else {
            let start = min(offset, groups.count)
            let end = min(start + limit, groups.count)
            return CatalogSearch.BrowsePage(
                groups: Array(groups[start..<end]),
                totalMatches: groups.count,
                offset: start,
                pageSize: limit
            )
        }

        var totalMatches = 0
        var pageGroups: [CatalogSearch.BrowseGroup] = []
        pageGroups.reserveCapacity(limit)
        for group in groups where group.searchKey.range(of: query) != nil {
            if totalMatches >= offset, pageGroups.count < limit { pageGroups.append(group) }
            totalMatches += 1
        }
        return CatalogSearch.BrowsePage(
            groups: pageGroups,
            totalMatches: totalMatches,
            offset: min(offset, totalMatches),
            pageSize: limit
        )
    }

    private func browseGroups(for mode: CatalogSearch.BrowseMode) -> [CatalogSearch.BrowseGroup] {
        switch mode {
        case .tracks:
            return []
        case .artists:
            if artistGroups == nil { artistGroups = makeArtistGroups() }
            return artistGroups ?? []
        case .albums:
            if albumGroups == nil { albumGroups = makeAlbumGroups() }
            return albumGroups ?? []
        }
    }

    private static func artistName(_ rawValue: String) -> String {
        let value = rawValue.trimmingCharacters(in: .whitespacesAndNewlines)
        return value.isEmpty ? "Unknown Artist" : value
    }

    private static func albumName(_ rawValue: String) -> String {
        let value = rawValue.trimmingCharacters(in: .whitespacesAndNewlines)
        return value.isEmpty ? "Unknown Album" : value
    }

    private static func artistBrowseKey(_ rawValue: String) -> String {
        CatalogTextNormalizer.fold(artistName(rawValue))
    }

    private func cachedArtistBrowseKeys() -> [String] {
        if foldedArtistKeys == nil {
            foldedArtistKeys = tracks.map { Self.artistBrowseKey($0.artist) }
        }
        return foldedArtistKeys ?? []
    }

    private func makeArtistGroups() -> [CatalogSearch.BrowseGroup] {
        struct Accumulator {
            var name: String
            var count: Int
        }
        var values: [String: Accumulator] = [:]
        let artistKeys = cachedArtistBrowseKeys()
        for (index, track) in tracks.enumerated() {
            let name = Self.artistName(track.artist)
            let key = artistKeys[index]
            if var value = values[key] {
                value.count += 1
                values[key] = value
            } else {
                values[key] = Accumulator(name: name, count: 1)
            }
        }
        return values.map { pair in
            let (key, value) = pair
            return CatalogSearch.BrowseGroup(
                id: key,
                name: value.name,
                trackCount: value.count,
                searchKey: key,
                detail: nil,
                artistFilterKey: key,
                albumFilterKey: nil
            )
        }.sorted { $0.id < $1.id }
    }

    private func makeAlbumGroups() -> [CatalogSearch.BrowseGroup] {
        struct Accumulator {
            var album: String
            var artist: String
            var artistKey: String
            var count: Int
        }
        var values: [String: Accumulator] = [:]
        let artistKeys = cachedArtistBrowseKeys()
        for (index, track) in tracks.enumerated() {
            let album = Self.albumName(track.album)
            let albumKey = CatalogTextNormalizer.fold(album)
            let artist = Self.artistName(track.artist)
            let artistKey = artistKeys[index]
            let id = "\(artistKey.utf8.count):\(artistKey)\(albumKey.utf8.count):\(albumKey)"
            if var value = values[id] {
                value.count += 1
                values[id] = value
            } else {
                values[id] = Accumulator(album: album, artist: artist, artistKey: artistKey, count: 1)
            }
        }
        return values.map { pair in
            let (id, value) = pair
            return CatalogSearch.BrowseGroup(
                id: id,
                name: value.album,
                trackCount: value.count,
                searchKey: CatalogTextNormalizer.fold(value.album + " " + value.artist),
                detail: value.artist,
                artistFilterKey: value.artistKey,
                albumFilterKey: CatalogTextNormalizer.fold(value.album)
            )
        }.sorted {
            let leftArtist = $0.artistFilterKey ?? ""
            let rightArtist = $1.artistFilterKey ?? ""
            if leftArtist != rightArtist { return leftArtist < rightArtist }
            return CatalogTextNormalizer.fold($0.name) < CatalogTextNormalizer.fold($1.name)
        }
    }

    private func sortedIndices(for order: CatalogSearch.SortOrder) -> [Int] {
        switch order {
        case .title:
            if titleOrder == nil { titleOrder = makeSortedIndices(order) }
            return titleOrder ?? []
        case .artist:
            if artistOrder == nil { artistOrder = makeSortedIndices(order) }
            return artistOrder ?? []
        case .album:
            if albumOrder == nil { albumOrder = makeSortedIndices(order) }
            return albumOrder ?? []
        }
    }

    private func makeSortedIndices(_ order: CatalogSearch.SortOrder) -> [Int] {
        var indices = Array(tracks.indices)
        indices.sort { comesBefore($0, $1, by: order) }
        return indices
    }

    private func comesBefore(_ leftIndex: Int, _ rightIndex: Int, by order: CatalogSearch.SortOrder) -> Bool {
        comesBefore(tracks[leftIndex], tracks[rightIndex], by: order)
    }

    private func comesBefore(_ left: CatalogTrack, _ right: CatalogTrack, by order: CatalogSearch.SortOrder) -> Bool {
        let leftKey: String
        let rightKey: String
        switch order {
        case .title: leftKey = left.titleSortKey; rightKey = right.titleSortKey
        case .artist: leftKey = left.artistSortKey; rightKey = right.artistSortKey
        case .album: leftKey = left.albumSortKey; rightKey = right.albumSortKey
        }
        return leftKey == rightKey ? left.id < right.id : leftKey < rightKey
    }
}

/// Owned exclusively by the serial catalog-view queue. Rebuilds only when the
/// active catalog revision changes; debounce cancellations can reuse a build.
final class CatalogSearchViewCache: @unchecked Sendable {
    private var revision: UInt64?
    private var index: CatalogSearchIndex?

    func visibleTracks(
        in tracks: [CatalogTrack],
        revision requestedRevision: UInt64,
        matching searchText: String,
        sortedBy order: CatalogSearch.SortOrder,
        in scope: CatalogSearch.SearchScope = .all
    ) -> [CatalogTrack] {
        if revision != requestedRevision {
            index = CatalogSearchIndex(tracks: tracks)
            revision = requestedRevision
        }
        return index?.visibleTracks(matching: searchText, sortedBy: order, in: scope) ?? []
    }

    func page(
        in tracks: [CatalogTrack],
        revision requestedRevision: UInt64,
        matching searchText: String,
        sortedBy order: CatalogSearch.SortOrder,
        in scope: CatalogSearch.SearchScope = .all,
        withinArtistKey: String? = nil,
        withinAlbumKey: String? = nil,
        offset: Int,
        limit: Int = CatalogSearch.maximumPageSize
    ) -> CatalogSearch.Page {
        if revision != requestedRevision {
            index = CatalogSearchIndex(tracks: tracks)
            revision = requestedRevision
        }
        return index?.page(
            matching: searchText,
            sortedBy: order,
            in: scope,
            withinArtistKey: withinArtistKey,
            withinAlbumKey: withinAlbumKey,
            offset: offset,
            limit: limit
        ) ?? CatalogSearch.Page(tracks: [], totalMatches: 0, offset: 0, pageSize: limit)
    }

    func browsePage(
        in tracks: [CatalogTrack],
        revision requestedRevision: UInt64,
        for mode: CatalogSearch.BrowseMode,
        matching searchText: String,
        offset: Int,
        limit: Int = CatalogSearch.maximumPageSize
    ) -> CatalogSearch.BrowsePage {
        if revision != requestedRevision {
            index = CatalogSearchIndex(tracks: tracks)
            revision = requestedRevision
        }
        return index?.browsePage(for: mode, matching: searchText, offset: offset, limit: limit)
            ?? CatalogSearch.BrowsePage(groups: [], totalMatches: 0, offset: 0, pageSize: limit)
    }
}

enum LibraryScanPresentation {
    static func message(isScanning: Bool, retainedTrackCount: Int) -> String? {
        guard isScanning, retainedTrackCount > 0 else { return nil }
        return "Scanning a folder. The previous catalog stays active; queue and playlist changes apply to it until this scan completes or is canceled."
    }
}
