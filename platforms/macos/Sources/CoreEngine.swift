import Foundation

struct VerifiedLexicon {
    fileprivate let data: Data

    init(data: Data, manifest: Data) throws {
        guard bpm_abi_version() == BPM_ABI_VERSION else { throw AdapterError.abi }
        guard manifest.count == 65, manifest.last == 0x0A else { throw AdapterError.integrity }
        let digest = Data(manifest.prefix(64))
        let status = data.withUnsafeBytes { bytes in
            digest.withUnsafeBytes { hash in
                bpm_lexicon_verify(bytes.bindMemory(to: UInt8.self).baseAddress, data.count,
                    hash.bindMemory(to: UInt8.self).baseAddress, digest.count)
            }
        }
        guard status == BPM_OK else { throw AdapterError.integrity }
        self.data = data
    }
}

final class CoreEngine {
    private let handle: OpaquePointer

    init(lexicon: VerifiedLexicon) throws {
        guard bpm_abi_version() == BPM_ABI_VERSION else { throw AdapterError.abi }
        var result: OpaquePointer?
        let status = lexicon.data.withUnsafeBytes { bytes in
            bpm_engine_new(bytes.bindMemory(to: UInt8.self).baseAddress, lexicon.data.count, &result)
        }
        guard status == BPM_OK, let result else { throw AdapterError.initialization }
        handle = result
    }

    deinit { bpm_engine_free(handle) }

    func send(_ event: EngineEvent) throws -> EngineOutput {
        let data: Data
        do { data = try JSONEncoder().encode(event) }
        catch { throw AdapterError.eventEncoding }
        var buffer = BpmBuffer(data: nil, len: 0)
        defer { bpm_buffer_free(buffer) }
        let status = data.withUnsafeBytes { bytes in
            bpm_engine_handle(handle, bytes.bindMemory(to: UInt8.self).baseAddress, data.count, &buffer)
        }
        guard status == BPM_OK else { throw AdapterError.ffi }
        let output = try decode(EngineOutput.self, buffer: buffer)
        try output.validateStatus()
        return output
    }

    func snapshot() throws -> Snapshot {
        var buffer = BpmBuffer(data: nil, len: 0)
        defer { bpm_buffer_free(buffer) }
        guard bpm_engine_snapshot(handle, &buffer) == BPM_OK else { throw AdapterError.ffi }
        return try decode(Snapshot.self, buffer: buffer)
    }

    private func decode<T: Decodable>(_ type: T.Type, buffer: BpmBuffer) throws -> T {
        guard let bytes = buffer.data, buffer.len > 0 else { throw AdapterError.outputDecoding }
        do { return try JSONDecoder().decode(type, from: Data(bytes: bytes, count: buffer.len)) }
        catch { throw AdapterError.outputDecoding }
    }
}

// Read once at process initialization. Key callbacks only use this in-memory data.
enum LexiconResource {
    static let result: Result<VerifiedLexicon, AdapterError> = {
        do {
            guard let dataURL = Bundle.main.url(forResource: "lexicon", withExtension: "tsv"),
                  let hashURL = Bundle.main.url(forResource: "lexicon", withExtension: "sha256") else {
                return .failure(.integrity)
            }
            let data = try Data(contentsOf: dataURL)
            let manifest = try Data(contentsOf: hashURL)
            return .success(try VerifiedLexicon(data: data, manifest: manifest))
        } catch let error as AdapterError { return .failure(error) }
        catch { return .failure(.initialization) }
    }()
}

func reportError(_ error: Error) {
    // Never format error descriptions: they can contain input text or JSON.
    let kind = (error as? AdapterError).map { String(describing: $0) } ?? "internal"
    NSLog("Bopomofo Development: error=%@", kind)
}
