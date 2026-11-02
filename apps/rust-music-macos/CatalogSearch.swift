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

    static func visibleTracks(
        in tracks: [CatalogTrack],
        matching searchText: String,
        sortedBy order: SortOrder = .title
    ) -> [CatalogTrack] {
        CatalogSearchIndex(tracks: tracks).visibleTracks(matching: searchText, sortedBy: order)
    }

    static func matches(_ track: CatalogTrack, matching searchText: String) -> Bool {
        guard let queryBytes = queryBytes(for: searchText) else { return true }
        return track.searchableBytes.range(of: queryBytes) != nil
    }

    static func isActionableSelectedTrack(
        _ track: CatalogTrack?,
        selectedTrackIDs: Set<Int>,
        matching searchText: String
    ) -> Bool {
        guard let track, selectedTrackIDs.contains(track.id) else { return false }
        return matches(track, matching: searchText)
    }

    static func mergingPageSelection(
        _ selectedTrackIDs: Set<Int>,
        visibleTrackIDs: Set<Int>,
        proposedVisibleSelection: Set<Int>
    ) -> Set<Int> {
        selectedTrackIDs.subtracting(visibleTrackIDs)
            .union(proposedVisibleSelection.intersection(visibleTrackIDs))
    }

    static func queryBytes(for searchText: String) -> Data? {
        let query = boundedQuery(searchText).trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return nil }
        return Data(CatalogTextNormalizer.fold(query).utf8)
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

/// Catalog-lifetime sort indexes. Queries filter an already ordered index, so
/// typing does not sort the full matching catalog again. Lazily builds each
/// requested order and keeps deterministic ID tie-breaking.
final class CatalogSearchIndex {
    private let tracks: [CatalogTrack]
    private let trackIDsMatchSourceOffsets: Bool
    private var titleOrder: [Int]?
    private var artistOrder: [Int]?
    private var albumOrder: [Int]?

    init(tracks: [CatalogTrack]) {
        self.tracks = tracks
        trackIDsMatchSourceOffsets = tracks.enumerated().allSatisfy { $0.offset == $0.element.id }
    }

    func visibleTracks(matching searchText: String, sortedBy order: CatalogSearch.SortOrder = .title) -> [CatalogTrack] {
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
        offset requestedOffset: Int,
        limit requestedLimit: Int = CatalogSearch.maximumPageSize
    ) -> CatalogSearch.Page {
        let limit = min(max(requestedLimit, 1), CatalogSearch.maximumPageSize)
        let offset = max(requestedOffset, 0)
        let queryBytes = CatalogSearch.queryBytes(for: searchText)
        let indices = sortedIndices(for: order)
        guard let queryBytes else {
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
        for index in indices where tracks[index].searchableBytes.range(of: queryBytes) != nil {
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
        sortedBy order: CatalogSearch.SortOrder
    ) -> [CatalogTrack] {
        if revision != requestedRevision {
            index = CatalogSearchIndex(tracks: tracks)
            revision = requestedRevision
        }
        return index?.visibleTracks(matching: searchText, sortedBy: order) ?? []
    }

    func page(
        in tracks: [CatalogTrack],
        revision requestedRevision: UInt64,
        matching searchText: String,
        sortedBy order: CatalogSearch.SortOrder,
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
            offset: offset,
            limit: limit
        ) ?? CatalogSearch.Page(tracks: [], totalMatches: 0, offset: 0, pageSize: limit)
    }
}

enum LibraryScanPresentation {
    static func message(isScanning: Bool, retainedTrackCount: Int) -> String? {
        guard isScanning, retainedTrackCount > 0 else { return nil }
        return "Scanning a folder. The previous catalog stays active; queue and playlist changes apply to it until this scan completes or is canceled."
    }
}
