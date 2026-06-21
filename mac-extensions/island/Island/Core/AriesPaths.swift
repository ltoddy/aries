import Foundation

enum AriesPaths {
    static var hooksDir: URL {
        FileManager.default.homeDirectoryForCurrentUser
            .appendingPathComponent(".agents/hooks/aries-island")
    }

    static var hooksFile: URL {
        hooksDir.appendingPathComponent("hooks.json")
    }

    static var hookScriptPath: URL {
        hooksDir.appendingPathComponent("aries-island-state.py")
    }

    static var hookScriptShellPath: String {
        shellQuote(hookScriptPath.path)
    }

    private static func shellQuote(_ path: String) -> String {
        "'" + path.replacingOccurrences(of: "'", with: "'\\''") + "'"
    }
}
