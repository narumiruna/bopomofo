import AppKit
import InputMethodKit

let application = NSApplication.shared
application.setActivationPolicy(.accessory)
// Force the resource load before the first input context or key callback.
if case .failure(let error) = LexiconResource.result {
    reportError(error)
    exit(1)
}
let bundle = Bundle.main
guard let identifier = bundle.bundleIdentifier,
      let connection = bundle.object(forInfoDictionaryKey: "InputMethodConnectionName") as? String,
      let server = IMKServer(name: connection, bundleIdentifier: identifier) else {
    reportError(AdapterError.initialization)
    exit(1)
}
withExtendedLifetime(server) { application.run() }
