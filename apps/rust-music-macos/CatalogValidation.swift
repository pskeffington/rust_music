import Foundation

enum CatalogValidation {
    static let maximumOutputBytes = 20 * 1024 * 1024

    static func acceptsIndex(_ index: Int, after parsedCount: Int, maximum: Int) -> Bool {
        parsedCount < maximum && index == parsedCount
    }

    static func readBounded(_ handle: FileHandle, maximumBytes: Int) throws -> Data? {
        var data = Data()
        while let chunk = try handle.read(upToCount: min(64 * 1024, maximumBytes + 1 - data.count)),
              !chunk.isEmpty {
            data.append(chunk)
            guard data.count <= maximumBytes else { return nil }
        }
        return data
    }
}
