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
        let query = boundedQuery(searchText).trimmingCharacters(in: .whitespacesAndNewlines)
        let filtered: [CatalogTrack]
        if query.isEmpty {
            filtered = tracks
        } else {
            let foldedQuery = CatalogTextNormalizer.fold(query)
            let queryBytes = Data(foldedQuery.utf8)
            filtered = tracks.filter { $0.searchableBytes.range(of: queryBytes) != nil }
        }
        return filtered.sorted { left, right in
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

    static func matches(_ track: CatalogTrack, matching searchText: String) -> Bool {
        let query = boundedQuery(searchText).trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return true }
        let queryBytes = Data(CatalogTextNormalizer.fold(query).utf8)
        return track.searchableBytes.range(of: queryBytes) != nil
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
