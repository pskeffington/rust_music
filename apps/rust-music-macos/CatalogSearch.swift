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
        let searchableText = [title, artist, album, relativePath]
            .map(CatalogTextNormalizer.fold)
            .joined(separator: "\u{0}")
        searchableBytes = Data(searchableText.utf8)
    }
}

enum CatalogSearch {
    static func visibleTracks(in tracks: [CatalogTrack], matching searchText: String) -> [CatalogTrack] {
        let query = boundedQuery(searchText).trimmingCharacters(in: .whitespacesAndNewlines)
        guard !query.isEmpty else { return tracks }
        let foldedQuery = CatalogTextNormalizer.fold(query)
        let queryBytes = Data(foldedQuery.utf8)
        return tracks.filter { $0.searchableBytes.range(of: queryBytes) != nil }
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
