import Foundation

@main
struct ValidateLibrary {
    static func main() {
        do {
            guard CommandLine.arguments.count == 3 else { throw AdapterError.initialization }
            let data = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[1]))
            let manifest = try Data(contentsOf: URL(fileURLWithPath: CommandLine.arguments[2]))
            let engine = try CoreEngine(assets: VerifiedAssets(data: data, manifest: manifest))
            let snapshot = try engine.snapshot()
            _ = try TextRange.utf16Offset(in: snapshot.preedit, utf8Offset: snapshot.cursor)
            print("Library ABI, JSON snapshot, and v2 asset integrity: OK")
        } catch {
            reportError(error)
            exit(1)
        }
    }
}
