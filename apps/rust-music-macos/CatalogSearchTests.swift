import Foundation

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

        // Measure the exact production filter with a warmed 20k in-memory catalog.
        // This is a microbenchmark only: no UI/accessibility overhead or pass threshold.
        _ = CatalogSearch.visibleTracks(in: tracks, matching: lastQuery)
        var durations: [UInt64] = []
        durations.reserveCapacity(101)
        for _ in 0..<101 {
            let start = DispatchTime.now().uptimeNanoseconds
            let result = CatalogSearch.visibleTracks(in: tracks, matching: lastQuery)
            let elapsed = DispatchTime.now().uptimeNanoseconds - start
            precondition(result.count == 1 && result[0].id == 19_999)
            durations.append(elapsed)
        }
        durations.sort()
        let p50 = Double(durations[(durations.count - 1) * 50 / 100]) / 1_000
        let p95 = Double(durations[(durations.count - 1) * 95 / 100]) / 1_000
        print(String(format:
            "PASS: production catalog search correctness; 20k last-item p50=%.3fµs p95=%.3fµs n=%d; cached-index payload=%d bytes (%.1f bytes/track)",
            p50,
            p95,
            durations.count,
            indexPayloadBytes,
            Double(indexPayloadBytes) / Double(tracks.count)
        ))
    }
}
