import Foundation

enum CatalogValidation {
    static let maximumOutputBytes = 20 * 1024 * 1024
    // Keep this in sync with MAX_STATE_TSV_BYTES in rust-music-cli.
    static let maximumStateOutputBytes = 6 * 1024 * 1024

    static func acceptsIndex(_ index: Int, after parsedCount: Int, maximum: Int) -> Bool {
        parsedCount < maximum && index == parsedCount
    }

    static func readBounded(
        _ handle: FileHandle,
        maximumBytes: Int,
        onFailure: () -> Void = {}
    ) throws -> Data? {
        precondition(maximumBytes >= 0 && maximumBytes < Int.max)
        var data = Data()
        do {
            while let chunk = try handle.read(upToCount: min(64 * 1024, maximumBytes + 1 - data.count)),
                  !chunk.isEmpty {
                data.append(chunk)
                guard data.count <= maximumBytes else {
                    onFailure()
                    return nil
                }
            }
        } catch {
            onFailure()
            throw error
        }
        return data
    }
}
