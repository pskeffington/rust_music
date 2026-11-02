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
        precondition(LibraryRowDensity.allCases.map(\.rawValue) == ["compact", "standard"])
        precondition(
            LibraryRowDensity.standard.titleFontSize == 12
                && LibraryRowDensity.standard.metadataFontSize == 10
                && LibraryRowDensity.standard.textSpacing == 2
                && LibraryRowDensity.standard.verticalPadding == 2,
            "standard density must retain the original Library typography and row spacing"
        )
        precondition(
            LibraryRowDensity.compact.titleFontSize < LibraryRowDensity.standard.titleFontSize
                && LibraryRowDensity.compact.metadataFontSize < LibraryRowDensity.standard.metadataFontSize
                && LibraryRowDensity.compact.textSpacing < LibraryRowDensity.standard.textSpacing
                && LibraryRowDensity.compact.verticalPadding < LibraryRowDensity.standard.verticalPadding,
            "compact density must reduce text and row spacing while preserving the same text styles"
        )

        let emptyNavigation = CatalogPageNavigation(totalItems: 0, offset: 0, visibleItemCount: 0)
        precondition(
            emptyNavigation.pageCount == 1
                && emptyNavigation.currentPage == 1
                && emptyNavigation.firstItem == 0
                && emptyNavigation.lastItem == 0
                && emptyNavigation.adjacentOffset(forward: false) == nil
                && emptyNavigation.adjacentOffset(forward: true) == nil,
            "an empty result set presents one inert page"
        )
        let middleNavigation = CatalogPageNavigation(totalItems: 401, offset: 200, visibleItemCount: 200)
        precondition(
            middleNavigation.pageCount == 3
                && middleNavigation.currentPage == 2
                && middleNavigation.firstItem == 201
                && middleNavigation.lastItem == 400
                && middleNavigation.adjacentOffset(forward: false) == 0
                && middleNavigation.adjacentOffset(forward: true) == 400
        )
        let lastNavigation = CatalogPageNavigation(totalItems: 401, offset: 400, visibleItemCount: 1)
        precondition(
            lastNavigation.currentPage == 3
                && lastNavigation.firstItem == 401
                && lastNavigation.lastItem == 401
                && lastNavigation.adjacentOffset(forward: false) == 200
                && lastNavigation.adjacentOffset(forward: true) == nil
        )
        let clampedNavigation = CatalogPageNavigation(totalItems: 401, offset: .max, visibleItemCount: 200)
        precondition(
            clampedNavigation.offset == 400
                && clampedNavigation.lastItem == 401
                && clampedNavigation.adjacentOffset(forward: true) == nil,
            "out-of-range offsets must not overflow page counters"
        )

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
        precondition(CatalogSearch.matches(unicodeTrack, matching: "JÓNSI", in: .artist))
        precondition(CatalogSearch.matches(unicodeTrack, matching: "o\u{0308}ST", in: .album))
        precondition(!CatalogSearch.matches(unicodeTrack, matching: "jÓNSI", in: .title))
        precondition(CatalogSearch.matches(unicodeTrack, matching: " ", in: .title))
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
        precondition(CatalogSearch.isActionableSelectedTrack(
            unicodeTrack,
            selectedTrackIDs: [unicodeTrack.id],
            matching: "Jónsi",
            in: .artist
        ))
        precondition(!CatalogSearch.isActionableSelectedTrack(
            unicodeTrack,
            selectedTrackIDs: [unicodeTrack.id],
            matching: "Jónsi",
            in: .album
        ), "selected-track actions must honor the active field scope")
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
            CatalogSearch.selectedTrackCountOutsideView(
                [0, 1, 2, 9],
                visibleTrackIDs: [0, 2, 3]
            ) == 2,
            "selection feedback must include rows hidden by search or pagination"
        )
        precondition(
            CatalogSearch.selectedTrackCountOutsideView(
                [0, 2],
                visibleTrackIDs: [0, 2, 3]
            ) == 0,
            "selection feedback must report zero hidden rows when all are visible"
        )
        let selectionAfterFirstPage = CatalogSearch.mergingPageSelection(
            [0, 201],
            visibleTrackIDs: [0, 1, 2],
            proposedVisibleSelection: [1, 2]
        )
        precondition(selectionAfterFirstPage == [1, 2, 201], "selection edits must retain IDs from other pages")
        let selectionAfterSecondPage = CatalogSearch.mergingPageSelection(
            selectionAfterFirstPage,
            visibleTrackIDs: [200, 201, 202],
            proposedVisibleSelection: [200, 201]
        )
        precondition(selectionAfterSecondPage == [1, 2, 200, 201], "page-local selection must not discard earlier-page rows")
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
        precondition(sortIndex.visibleTracks(matching: "Able", in: .title).isEmpty)
        precondition(sortIndex.visibleTracks(matching: "Able", sortedBy: .title, in: .artist).map(\.id) == [0, 1])
        precondition(sortIndex.visibleTracks(matching: "Three", in: .album).map(\.id) == [0])
        precondition(sortIndex.visibleTracks(matching: "", sortedBy: .album).map(\.id) == [1, 0, 2])
        precondition(sortIndex.visibleTracks(matching: "Beta", sortedBy: .album).map(\.id) == [2])

        let artistGroups = sortIndex.browsePage(for: .artists, matching: "", offset: 0, limit: 20)
        precondition(
            artistGroups.groups.map(\.name) == ["Able", "Zulu"]
                && artistGroups.groups.map(\.trackCount) == [2, 1]
                && artistGroups.totalMatches == 2,
            "artist browsing deduplicates names and reports track counts"
        )
        let nextArtistGroup = sortIndex.browsePage(for: .artists, matching: "", offset: 1, limit: 1)
        precondition(nextArtistGroup.groups.map(\.name) == ["Zulu"] && nextArtistGroup.totalMatches == 2)
        let searchedAlbums = sortIndex.browsePage(for: .albums, matching: "thr", offset: 0, limit: 20)
        precondition(
            searchedAlbums.groups.map(\.name) == ["Three"] && searchedAlbums.totalMatches == 1,
            "album browsing searches normalized album names"
        )
        let sameTitleAlbumIndex = CatalogSearchIndex(tracks: [
            CatalogTrack(id: 0, container: "flac", title: "First", artist: "Artist One", album: "Same Title", relativePath: "one-1.flac"),
            CatalogTrack(id: 1, container: "flac", title: "Second", artist: "Artist One", album: "Same Title", relativePath: "one-2.flac"),
            CatalogTrack(id: 2, container: "flac", title: "Third", artist: "Artist Two", album: "Same Title", relativePath: "two.flac"),
            CatalogTrack(id: 3, container: "flac", title: "Fourth", artist: "artist one", album: "same title", relativePath: "one-3.flac")
        ])
        let sameTitleAlbums = sameTitleAlbumIndex.browsePage(for: .albums, matching: "same title", offset: 0)
        precondition(
            sameTitleAlbums.totalMatches == 2
                && sameTitleAlbums.groups.map(\.detail) == ["Artist One", "Artist Two"]
                && sameTitleAlbums.groups.map(\.trackCount) == [3, 1]
                && sameTitleAlbums.groups[0].id != sameTitleAlbums.groups[1].id,
            "same-titled albums stay distinct by normalized artist while case variants merge"
        )
        let artistLimitedAlbumPage = sameTitleAlbumIndex.page(
            matching: "",
            withinArtistKey: "artist two",
            withinAlbumKey: "same title",
            offset: 0
        )
        precondition(
            artistLimitedAlbumPage.tracks.map(\.id) == [2]
                && artistLimitedAlbumPage.totalMatches == 1,
            "opening an album group constrains its track page to the group artist"
        )
        let artistSearchAlbum = sameTitleAlbumIndex.browsePage(for: .albums, matching: "Artist Two", offset: 0)
        precondition(artistSearchAlbum.groups.map(\.detail) == ["Artist Two"])
        let unknownTagGroups = CatalogSearchIndex(tracks: [
            CatalogTrack(id: 0, container: "flac", title: "A", artist: "Björk", album: "", relativePath: "a.flac"),
            CatalogTrack(id: 1, container: "flac", title: "B", artist: "björk", album: "Unknown Album", relativePath: "b.flac"),
            CatalogTrack(id: 2, container: "flac", title: "C", artist: "", album: "", relativePath: "c.flac")
        ])
        let normalizedArtistGroups = unknownTagGroups.browsePage(for: .artists, matching: "jÓNSI", offset: 0)
        precondition(normalizedArtistGroups.groups.isEmpty)
        let deduplicatedArtists = unknownTagGroups.browsePage(for: .artists, matching: "bjÖRK", offset: 0)
        precondition(
            deduplicatedArtists.groups.count == 1
                && deduplicatedArtists.groups[0].trackCount == 2
                && deduplicatedArtists.groups[0].name == "Björk",
            "artist group keys use canonical case-folded metadata"
        )
        let unknownAlbums = unknownTagGroups.browsePage(for: .albums, matching: "unknown", offset: 0)
        precondition(
            unknownAlbums.groups.count == 2
                && unknownAlbums.groups.map(\.trackCount).sorted() == [1, 2],
            "unknown albums are grouped separately by artist"
        )
        let unknownAlbumGroup = unknownAlbums.groups.first { $0.detail == "Björk" }
        precondition(unknownAlbumGroup?.albumFilterKey == "unknown album")
        let unknownArtistAndAlbumPage = unknownTagGroups.page(
            matching: "",
            withinArtistKey: "unknown artist",
            withinAlbumKey: "unknown album",
            offset: 0
        )
        precondition(
            unknownArtistAndAlbumPage.tracks.map(\.id) == [2],
            "opening missing-tag artist and album groups finds tracks with blank metadata"
        )
        let unknownArtists = unknownTagGroups.browsePage(for: .artists, matching: "unknown", offset: 0)
        precondition(unknownArtists.groups.count == 1 && unknownArtists.groups[0].trackCount == 1)
        let boundedGroupPage = CatalogSearchIndex(tracks: (0..<1_000).map { index in
            CatalogTrack(
                id: index,
                container: "flac",
                title: "Track \(index)",
                artist: "Artist \(index)",
                album: "Album \(index)",
                relativePath: "\(index).flac"
            )
        }).browsePage(for: .artists, matching: "", offset: 0, limit: 10_000)
        precondition(
            boundedGroupPage.groups.count == CatalogSearch.maximumPageSize
                && boundedGroupPage.totalMatches == 1_000,
            "artist and album browser pages obey the existing 200-row bound"
        )

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

        let pageIndex = CatalogSearchIndex(tracks: sortTracks)
        let sortedPage = pageIndex.page(matching: "", sortedBy: .title, offset: 1, limit: 1)
        precondition(sortedPage.tracks.map(\.id) == [1] && sortedPage.totalMatches == 3 && sortedPage.offset == 1)
        let filteredPage = pageIndex.page(matching: "Able", sortedBy: .artist, offset: 1, limit: 200)
        precondition(filteredPage.tracks.map(\.id) == [1] && filteredPage.totalMatches == 2)
        let scopedPage = pageIndex.page(matching: "Able", sortedBy: .title, in: .artist, offset: 1, limit: 1)
        precondition(
            scopedPage.tracks.map(\.id) == [1] && scopedPage.totalMatches == 2 && scopedPage.offset == 1,
            "field-scoped pagination must filter, sort, and count consistently"
        )
        let titleDoesNotMatchArtist = pageIndex.page(matching: "Able", in: .title, offset: 0, limit: 5)
        precondition(titleDoesNotMatchArtist.tracks.isEmpty && titleDoesNotMatchArtist.totalMatches == 0)
        let beyondEndPage = pageIndex.page(matching: "Beta", sortedBy: .album, offset: 10, limit: 1)
        precondition(beyondEndPage.tracks.isEmpty && beyondEndPage.totalMatches == 1 && beyondEndPage.offset == 1)
        let pageCache = CatalogSearchViewCache()
        precondition(pageCache.page(
            in: sortTracks,
            revision: 11,
            matching: "",
            sortedBy: .title,
            offset: 0,
            limit: 2
        ).tracks.map(\.id) == [0, 1])
        precondition(pageCache.page(
            in: replacementCatalog,
            revision: 12,
            matching: "",
            sortedBy: .title,
            offset: 0,
            limit: 2
        ).tracks.map(\.id) == [9], "page cache must invalidate on catalog revision changes")
        precondition(pageCache.page(
            in: sortTracks,
            revision: 11,
            matching: "Zulu",
            sortedBy: .artist,
            in: .artist,
            offset: 0,
            limit: 5
        ).tracks.map(\.id) == [2], "the view cache must pass through field scope")
        precondition(pageCache.browsePage(
            in: replacementCatalog,
            revision: 12,
            for: .artists,
            matching: "Artist",
            offset: 0,
            limit: 5
        ).groups.map(\.name) == ["Artist"], "the view cache must invalidate grouped indexes by catalog revision")

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
        let cappedPage = CatalogSearchIndex(tracks: tracks).page(
            matching: "",
            sortedBy: .title,
            offset: 0,
            limit: 10_000
        )
        precondition(
            cappedPage.tracks.count == CatalogSearch.maximumPageSize && cappedPage.totalMatches == 20_000,
            "page materialization must stay within the 200-row limit"
        )
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
                artist: String(format: "Synthetic Artist %06d", index),
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
        var pageDurations: [UInt64] = []
        pageDurations.reserveCapacity(21)
        for _ in 0..<21 {
            let start = DispatchTime.now().uptimeNanoseconds
            let page = index.page(
                matching: "synthetic artist",
                sortedBy: .title,
                offset: count / 2,
                limit: 10_000
            )
            pageDurations.append(DispatchTime.now().uptimeNanoseconds - start)
            precondition(
                page.totalMatches == count
                    && page.tracks.count == min(CatalogSearch.maximumPageSize, count - count / 2),
                "broad-query pages report full count and materialize no more than 200 rows"
            )
        }
        pageDurations.sort()
        let broadPageP95 = Double(pageDurations[19]) / 1_000_000
        let artistIndexStart = DispatchTime.now().uptimeNanoseconds
        let artistGroups = index.browsePage(
            for: .artists,
            matching: "",
            offset: count / 2,
            limit: 10_000
        )
        let artistIndexMilliseconds = Double(DispatchTime.now().uptimeNanoseconds - artistIndexStart) / 1_000_000
        precondition(
            artistGroups.totalMatches == count
                && artistGroups.groups.count == min(CatalogSearch.maximumPageSize, count - count / 2),
            "artist browsing must index all groups while returning no more than 200 rows"
        )
        let artistQuery = String(format: "synthetic artist %06d", count - 1)
        var artistSearchDurations: [UInt64] = []
        artistSearchDurations.reserveCapacity(5)
        for _ in 0..<5 {
            let start = DispatchTime.now().uptimeNanoseconds
            let page = index.browsePage(for: .artists, matching: artistQuery, offset: 0)
            artistSearchDurations.append(DispatchTime.now().uptimeNanoseconds - start)
            precondition(
                page.totalMatches == 1 && page.groups.first?.name == String(format: "Synthetic Artist %06d", count - 1),
                "artist query must find its matching group and remain bounded"
            )
        }
        artistSearchDurations.sort()
        let artistSearchP95 = Double(artistSearchDurations[4]) / 1_000_000
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
            "PASS: synthetic catalog scale n=%d; title-index-build=%.3fms; cached-sort p50=%.3fms p95=%.3fms; narrow-search p50=%.3fms p95=%.3fms linear-reference-p95=%.3fms; broad-search p50=%.3fms p95=%.3fms; bounded-200-row broad-page p95=%.3fms; artist-groups=%d index+page=%.3fms narrow-query-p95=%.3fms; searchable payload=%d bytes",
            count,
            indexBuildMilliseconds,
            sortP50,
            sortP95,
            searchP50,
            searchP95,
            linearSearchP95,
            broadSearchP50,
            broadSearchP95,
            broadPageP95,
            artistGroups.totalMatches,
            artistIndexMilliseconds,
            artistSearchP95,
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
