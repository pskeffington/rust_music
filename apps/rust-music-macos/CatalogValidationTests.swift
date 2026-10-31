import Foundation

@main
struct CatalogValidationTests {
    static func main() {
        precondition(CatalogValidation.maximumStateOutputBytes == 6 * 1024 * 1024)
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

        let exactStateLimit = Pipe()
        let exactBytes = Data(repeating: 0x41, count: CatalogValidation.maximumStateOutputBytes)
        let writerFinished = DispatchSemaphore(value: 0)
        DispatchQueue.global().async {
            defer { writerFinished.signal() }
            try! exactStateLimit.fileHandleForWriting.write(contentsOf: exactBytes)
            try! exactStateLimit.fileHandleForWriting.close()
        }
        precondition(try! CatalogValidation.readBounded(
            exactStateLimit.fileHandleForReading,
            maximumBytes: CatalogValidation.maximumStateOutputBytes
        )?.count == CatalogValidation.maximumStateOutputBytes)
        precondition(writerFinished.wait(timeout: .now() + 5) == .success)

        let failedRead = Pipe()
        try! failedRead.fileHandleForReading.close()
        var cancelledFailedRead = false
        do {
            _ = try CatalogValidation.readBounded(
                failedRead.fileHandleForReading,
                maximumBytes: 4,
                onFailure: { cancelledFailedRead = true }
            )
            preconditionFailure("closed pipe should fail")
        } catch {
            precondition(cancelledFailedRead)
        }

        let overProducingChild = Process()
        overProducingChild.executableURL = URL(fileURLWithPath: "/usr/bin/yes")
        let childOutput = Pipe()
        overProducingChild.standardOutput = childOutput
        try! overProducingChild.run()
        var cancelledOverProducer = false
        let oversizedChildOutput = try! CatalogValidation.readBounded(
            childOutput.fileHandleForReading,
            maximumBytes: 1024,
            onFailure: {
                cancelledOverProducer = true
                if overProducingChild.isRunning { overProducingChild.terminate() }
            }
        )
        if overProducingChild.isRunning { overProducingChild.terminate() }
        overProducingChild.waitUntilExit()
        precondition(oversizedChildOutput == nil)
        precondition(cancelledOverProducer)
        precondition(!overProducingChild.isRunning)
        print("PASS: catalog indexes and streamed output enforce configured bounds")
    }
}
