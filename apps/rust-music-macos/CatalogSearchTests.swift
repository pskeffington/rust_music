import Foundation
import Darwin

private struct UnindexedCatalogTrack {
    let id: Int
    let container: String
    let title: String
    let artist: String
    let album: String
    let relativePath: String
}

@main
struct CatalogSearchTests {
    static func main() {
        let unicodeTrack = CatalogTrack(
            id: 0,
            container: "flac",
            title: "Björk",
            artist: "Jónsi",
            album: "Östergötland",
            relativePath: "Iceland/Östergötland.flac"
        )
        let unicodeMatches = CatalogSearch.visibleTracks(in: [unicodeTrack], matching: "öSTERGÖTLAND")
        precondition(unicodeMatches == [unicodeTrack], "search must match Unicode case-insensitively")
        precondition(CatalogSearch.matches(unicodeTrack, matching: "östergötland"))
        precondition(
            CatalogSearch.matches(unicodeTrack, matching: "o\u{0308}STERGÖTLAND"),
            "filtered-play guard must use the same canonical Unicode matching as the visible list"
        )
        precondition(CatalogSearch.matches(unicodeTrack, matching: " "))
        precondition(!CatalogSearch.matches(unicodeTrack, matching: "not-present"))
        precondition(
            LibraryScanPresentation.message(isScanning: true, retainedTrackCount: 3)
                == "Scanning a folder. The previous catalog stays active; queue and playlist changes apply to it until this scan completes or is canceled."
        )
        precondition(LibraryScanPresentation.message(isScanning: true, retainedTrackCount: 0) == nil)
        precondition(LibraryScanPresentation.message(isScanning: false, retainedTrackCount: 3) == nil)
        precondition(
            CatalogSearch.isActionableSelectedTrack(
                unicodeTrack,
                selectedTrackIDs: [unicodeTrack.id],
                matching: "östergötland"
            ),
            "selected-track actions require a selected row that remains visible in search"
        )
        precondition(
            !CatalogSearch.isActionableSelectedTrack(
                unicodeTrack,
                selectedTrackIDs: [],
                matching: ""
            ),
            "deselecting the row must disable actions labelled for the selected track"
        )
        precondition(
            !CatalogSearch.isActionableSelectedTrack(
                unicodeTrack,
                selectedTrackIDs: [unicodeTrack.id],
                matching: "not-present"
            ),
            "a selected row hidden by search must not remain actionable"
        )
        precondition(
            CatalogSearch.visibleTracks(in: [unicodeTrack], matching: "o\u{0308}STERGÖTLAND") == [unicodeTrack]
        )
        precondition(CatalogSearch.visibleTracks(in: [unicodeTrack], matching: "jÓNSI") == [unicodeTrack])
        precondition(CatalogSearch.visibleTracks(in: [unicodeTrack], matching: "iceland") == [unicodeTrack])
        precondition(CatalogSearch.visibleTracks(in: [unicodeTrack], matching: "bj\u{0000}\u{0085}örk") == [unicodeTrack])
        precondition(CatalogSearch.visibleTracks(in: [unicodeTrack], matching: "\u{0000}") == [unicodeTrack])
        precondition(CatalogSearch.visibleTracks(in: [unicodeTrack], matching: String(repeating: "x", count: 512) + "björk").isEmpty)
        precondition(CatalogSearch.visibleTracks(in: [unicodeTrack], matching: " \t\n") == [unicodeTrack])
        precondition(CatalogSearch.visibleTracks(in: [unicodeTrack], matching: "not-present").isEmpty)

        let sortTracks = [
            CatalogTrack(id: 2, container: "flac", title: "Beta", artist: "Zulu", album: "Two", relativePath: "b.flac"),
            CatalogTrack(id: 0, container: "flac", title: "alpha", artist: "Able", album: "Three", relativePath: "a.flac"),
            CatalogTrack(id: 1, container: "flac", title: "Alpha", artist: "Able", album: "One", relativePath: "c.flac")
        ]
        let sortIndex = CatalogSearchIndex(tracks: sortTracks)
        precondition(sortIndex.visibleTracks(matching: "").map(\.id) == [0, 1, 2])
        precondition(sortIndex.visibleTracks(matching: "Able", sortedBy: .artist).map(\.id) == [0, 1])
        precondition(sortIndex.visibleTracks(matching: "", sortedBy: .album).map(\.id) == [1, 0, 2])
        precondition(sortIndex.visibleTracks(matching: "Beta", sortedBy: .album).map(\.id) == [2])

        let viewCache = CatalogSearchViewCache()
        precondition(viewCache.visibleTracks(
            in: sortTracks,
            revision: 1,
            matching: "",
            sortedBy: .title
        ).map(\.id) == [0, 1, 2])
        let replacementCatalog = [
            CatalogTrack(id: 9, container: "flac", title: "New", artist: "Artist", album: "New", relativePath: "new.flac")
        ]
        precondition(viewCache.visibleTracks(
            in: replacementCatalog,
            revision: 2,
            matching: "",
            sortedBy: .title
        ).map(\.id) == [9], "catalog revision must discard indexes from the previous root")

        let tracks = (0..<20_000).map { index in
            let title = String(format: "Track %05d", index)
            return CatalogTrack(
                id: index,
                container: "wave",
                title: title,
                artist: "Synthetic Artist",
                album: "Scale Profile",
                relativePath: "folder/\(title).wav"
            )
        }
        precondition(CatalogSearch.visibleTracks(in: tracks, matching: "").count == 20_000)
        let lastQuery = "track 19999"
        let lastMatches = CatalogSearch.visibleTracks(in: tracks, matching: lastQuery)
        precondition(lastMatches.count == 1 && lastMatches[0].id == 19_999)
        let indexPayloadBytes = tracks.reduce(0) { $0 + $1.searchableBytes.count }

        let inputs = (0..<20_000).map { index in
            let title = String(format: "Track %05d", index)
            return UnindexedCatalogTrack(
                id: index,
                container: "wave",
                title: title,
                artist: "Synthetic Artist",
                album: "Scale Profile",
                relativePath: "folder/\(title).wav"
            )
        }
        var plainBuilds: [UInt64] = []
        var indexedBuilds: [UInt64] = []
        for _ in 0..<9 {
            let plainStart = DispatchTime.now().uptimeNanoseconds
            let plain = inputs.map {
                UnindexedCatalogTrack(
                    id: $0.id,
                    container: $0.container,
                    title: $0.title,
                    artist: $0.artist,
                    album: $0.album,
                    relativePath: $0.relativePath
                )
            }
            plainBuilds.append(DispatchTime.now().uptimeNanoseconds - plainStart)
            precondition(plain.count == 20_000 && plain.last?.id == 19_999)
            withExtendedLifetime(plain) {}

            let indexedStart = DispatchTime.now().uptimeNanoseconds
            let indexed = inputs.map {
                CatalogTrack(
                    id: $0.id,
                    container: $0.container,
                    title: $0.title,
                    artist: $0.artist,
                    album: $0.album,
                    relativePath: $0.relativePath
                )
            }
            indexedBuilds.append(DispatchTime.now().uptimeNanoseconds - indexedStart)
            precondition(indexed.count == 20_000 && indexed.last?.id == 19_999)
            withExtendedLifetime(indexed) {}
        }
        plainBuilds.sort()
        indexedBuilds.sort()
        let plainBuildP50 = Double(plainBuilds[4]) / 1_000_000
        let plainBuildP95 = Double(plainBuilds[8]) / 1_000_000
        let indexedBuildP50 = Double(indexedBuilds[4]) / 1_000_000
        let indexedBuildP95 = Double(indexedBuilds[8]) / 1_000_000

        // Measure the exact production filter with a warmed 20k in-memory catalog.
        // This is a microbenchmark only: no UI/accessibility overhead or pass threshold.
        let searchIndex = CatalogSearchIndex(tracks: tracks)
        _ = searchIndex.visibleTracks(matching: lastQuery)
        var durations: [UInt64] = []
        durations.reserveCapacity(101)
        for _ in 0..<101 {
            let start = DispatchTime.now().uptimeNanoseconds
            let result = searchIndex.visibleTracks(matching: lastQuery)
            let elapsed = DispatchTime.now().uptimeNanoseconds - start
            precondition(result.count == 1 && result[0].id == 19_999)
            durations.append(elapsed)
        }
        durations.sort()
        let broadResult = searchIndex.visibleTracks(matching: "synthetic artist")
        precondition(broadResult.count == 20_000 && broadResult.first?.id == 0 && broadResult.last?.id == 19_999)
        let p50 = Double(durations[(durations.count - 1) * 50 / 100]) / 1_000
        let p95 = Double(durations[(durations.count - 1) * 95 / 100]) / 1_000
        print(String(format:
            "PASS: production catalog search correctness; 20k last-item p50=%.3fµs p95=%.3fµs n=%d; model-build 20k unindexed p50=%.3fms p95=%.3fms indexed p50=%.3fms p95=%.3fms; cached-index payload=%d bytes (%.1f bytes/track)",
            p50,
            p95,
            durations.count,
            plainBuildP50,
            plainBuildP95,
            indexedBuildP50,
            indexedBuildP95,
            indexPayloadBytes,
            Double(indexPayloadBytes) / Double(tracks.count)
        ))

        for count in [2_000, 20_000, 100_000, 250_000] {
            let profile = scaleProfile(count: count)
            print(profile)
        }
        var resourceUsage = rusage()
        precondition(getrusage(RUSAGE_SELF, &resourceUsage) == 0)
        print("PASS: catalog test process peak RSS=\(resourceUsage.ru_maxrss) bytes")
    }

    private static func scaleProfile(count: Int) -> String {
        let tracks = (0..<count).map { index in
            // Keep persistent IDs aligned with catalog offsets while making
            // title order nontrivial, like a scanned path-ordered library.
            let title = String(format: "Track %06d", (index * 7_919) % count)
            return CatalogTrack(
                id: index,
                container: "wave",
                title: title,
                artist: "Synthetic Artist",
                album: "Scale Profile",
                relativePath: "folder/\(title).wav"
            )
        }
        let targetID = count - 1
        let query = String(format: "track %06d", targetID)
        let target = tracks.first(where: { $0.title == String(format: "Track %06d", targetID) })
        precondition(target != nil)
        let index = CatalogSearchIndex(tracks: tracks)
        let indexBuildStart = DispatchTime.now().uptimeNanoseconds
        let sortedTracks = index.visibleTracks(matching: "", sortedBy: .title)
        let indexBuildMilliseconds = Double(DispatchTime.now().uptimeNanoseconds - indexBuildStart) / 1_000_000
        precondition(
            sortedTracks.count == count
                && sortedTracks.first?.title == "Track 000000"
                && sortedTracks.last?.title == String(format: "Track %06d", targetID)
        )

        var sortDurations: [UInt64] = []
        sortDurations.reserveCapacity(21)
        for _ in 0..<21 {
            let start = DispatchTime.now().uptimeNanoseconds
            let result = index.visibleTracks(matching: "", sortedBy: .title)
            sortDurations.append(DispatchTime.now().uptimeNanoseconds - start)
            precondition(
                result.count == count
                    && result.first?.title == "Track 000000"
                    && result.last?.title == String(format: "Track %06d", targetID)
            )
        }
        sortDurations.sort()
        let sortP50 = Double(sortDurations[10]) / 1_000_000
        let sortP95 = Double(sortDurations[19]) / 1_000_000

        var searchDurations: [UInt64] = []
        searchDurations.reserveCapacity(21)
        for _ in 0..<21 {
            let start = DispatchTime.now().uptimeNanoseconds
            let result = index.visibleTracks(matching: query, sortedBy: .title)
            searchDurations.append(DispatchTime.now().uptimeNanoseconds - start)
            precondition(result.count == 1 && result[0].title == target?.title)
        }
        searchDurations.sort()
        let searchP50 = Double(searchDurations[10]) / 1_000_000
        let searchP95 = Double(searchDurations[19]) / 1_000_000
        var linearSearchDurations: [UInt64] = []
        linearSearchDurations.reserveCapacity(5)
        for _ in 0..<5 {
            let start = DispatchTime.now().uptimeNanoseconds
            let result = linearVisibleTracks(in: tracks, matching: query, sortedBy: .title)
            linearSearchDurations.append(DispatchTime.now().uptimeNanoseconds - start)
            precondition(result.count == 1 && result[0].title == target?.title)
        }
        linearSearchDurations.sort()
        let linearSearchP95 = Double(linearSearchDurations[4]) / 1_000_000
        var broadSearchDurations: [UInt64] = []
        broadSearchDurations.reserveCapacity(5)
        for _ in 0..<5 {
            let start = DispatchTime.now().uptimeNanoseconds
            let result = index.visibleTracks(matching: "synthetic artist", sortedBy: .title)
            broadSearchDurations.append(DispatchTime.now().uptimeNanoseconds - start)
            precondition(
                result.count == count
                    && result.first?.title == "Track 000000"
                    && result.last?.title == String(format: "Track %06d", targetID)
            )
        }
        broadSearchDurations.sort()
        let broadSearchP50 = Double(broadSearchDurations[2]) / 1_000_000
        let broadSearchP95 = Double(broadSearchDurations[4]) / 1_000_000
        let indexPayloadBytes = tracks.reduce(0) { $0 + $1.searchableBytes.count }
        return String(format:
            "PASS: synthetic catalog scale n=%d; title-index-build=%.3fms; cached-sort p50=%.3fms p95=%.3fms; narrow-search p50=%.3fms p95=%.3fms linear-reference-p95=%.3fms; broad-search p50=%.3fms p95=%.3fms; searchable payload=%d bytes",
            count,
            indexBuildMilliseconds,
            sortP50,
            sortP95,
            searchP50,
            searchP95,
            linearSearchP95,
            broadSearchP50,
            broadSearchP95,
            indexPayloadBytes
        )
    }

    private static func linearVisibleTracks(
        in tracks: [CatalogTrack],
        matching searchText: String,
        sortedBy order: CatalogSearch.SortOrder
    ) -> [CatalogTrack] {
        let queryBytes = CatalogSearch.queryBytes(for: searchText)
        let filtered = tracks.filter { track in
            guard let queryBytes else { return true }
            return track.searchableBytes.range(of: queryBytes) != nil
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
}
