import Foundation

struct VerifiedAssets {
    fileprivate let data: Data
    fileprivate let manifest: Data

    init(data: Data, manifest: Data) throws {
        guard bpm_abi_version() == BPM_ABI_VERSION else { throw AdapterError.abi }
        guard !data.isEmpty, data.count <= 32 * 1024 * 1024,
              !manifest.isEmpty, manifest.count <= 16 * 1024 else { throw AdapterError.integrity }
        self.data = data
        self.manifest = manifest
        let status = withBuffers { manifest, length, asset in
            bpm_assets_verify(manifest, length, asset, 1)
        }
        guard status == BPM_OK else { throw AdapterError.integrity }
    }

    fileprivate func withBuffers<T>(_ body: (UnsafePointer<UInt8>?, Int, UnsafePointer<BpmAsset>) -> T) -> T {
        let name = Data("dictionary.dat".utf8)
        return data.withUnsafeBytes { dataBytes in
            manifest.withUnsafeBytes { manifestBytes in
                name.withUnsafeBytes { nameBytes in
                    var asset = BpmAsset(name: nameBytes.bindMemory(to: UInt8.self).baseAddress,
                        name_len: name.count, data: dataBytes.bindMemory(to: UInt8.self).baseAddress,
                        len: data.count)
                    return withUnsafePointer(to: &asset) { pointer in
                        body(manifestBytes.bindMemory(to: UInt8.self).baseAddress, manifest.count, pointer)
                    }
                }
            }
        }
    }

    static func read(_ url: URL, maximum: Int) throws -> Data {
        let file = try FileHandle(forReadingFrom: url)
        defer { try? file.close() }
        let bytes = try file.read(upToCount: maximum + 1) ?? Data()
        guard !bytes.isEmpty, bytes.count <= maximum else { throw AdapterError.integrity }
        return bytes
    }
}

final class CoreEngine {
    private let handle: OpaquePointer

    init(assets: VerifiedAssets) throws {
        guard bpm_abi_version() == BPM_ABI_VERSION else { throw AdapterError.abi }
        var result: OpaquePointer?
        let status = assets.withBuffers { manifest, length, asset in
            bpm_engine_new_v2(manifest, length, asset, 1, &result)
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

// Read once at process initialization. Key callbacks use only verified memory.
enum AssetResource {
    static let result: Result<VerifiedAssets, AdapterError> = {
        do {
            guard let dataURL = Bundle.main.url(forResource: "dictionary", withExtension: "dat"),
                  let manifestURL = Bundle.main.url(forResource: "assets", withExtension: "json") else {
                return .failure(.integrity)
            }
            let data = try VerifiedAssets.read(dataURL, maximum: 32 * 1024 * 1024)
            let manifest = try VerifiedAssets.read(manifestURL, maximum: 16 * 1024)
            return .success(try VerifiedAssets(data: data, manifest: manifest))
        } catch let error as AdapterError { return .failure(error) }
        catch { return .failure(.initialization) }
    }()
}

func reportError(_ error: Error) {
    // Error descriptions can contain input text or JSON. Emit only fixed kinds.
    let kind = (error as? AdapterError).map { String(describing: $0) } ?? "internal"
    NSLog("Bopomofo Development: error=%@", kind)
}
