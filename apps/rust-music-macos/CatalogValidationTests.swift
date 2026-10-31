import Foundation

@main
struct CatalogValidationTests {
    static func main() {
        precondition(CatalogValidation.acceptsIndex(0, after: 0, maximum: 20_000))
        precondition(CatalogValidation.acceptsIndex(1, after: 1, maximum: 20_000))
        precondition(!CatalogValidation.acceptsIndex(2, after: 1, maximum: 20_000)) // gap
        precondition(!CatalogValidation.acceptsIndex(0, after: 1, maximum: 20_000)) // duplicate
        precondition(!CatalogValidation.acceptsIndex(1, after: 0, maximum: 20_000)) // permutation
        precondition(!CatalogValidation.acceptsIndex(20_000, after: 20_000, maximum: 20_000))

        let withinBound = Pipe()
        try! withinBound.fileHandleForWriting.write(contentsOf: Data(repeating: 0x41, count: 4))
        try! withinBound.fileHandleForWriting.close()
        precondition(try! CatalogValidation.readBounded(withinBound.fileHandleForReading, maximumBytes: 4)?.count == 4)

        let overBound = Pipe()
        try! overBound.fileHandleForWriting.write(contentsOf: Data(repeating: 0x41, count: 5))
        try! overBound.fileHandleForWriting.close()
        precondition(try! CatalogValidation.readBounded(overBound.fileHandleForReading, maximumBytes: 4) == nil)
        print("PASS: catalog indexes and streamed output enforce configured bounds")
    }
}
