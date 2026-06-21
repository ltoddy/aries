import Foundation

enum AriesHookInstaller {
    static func installIfNeeded() {
        let hooksDir = AriesPaths.hooksDir
        let pythonScript = AriesPaths.hookScriptPath

        try? FileManager.default.createDirectory(
            at: hooksDir,
            withIntermediateDirectories: true
        )

        if let bundled = Bundle.main.url(forResource: "aries-island-state", withExtension: "py") {
            try? FileManager.default.removeItem(at: pythonScript)
            try? FileManager.default.copyItem(at: bundled, to: pythonScript)
            try? FileManager.default.setAttributes(
                [.posixPermissions: 0o755],
                ofItemAtPath: pythonScript.path
            )
        }

        createHooksConfig(at: AriesPaths.hooksFile)
    }

    private static func createHooksConfig(at hooksFile: URL) {
        let python = detectPython()
        let command = "\(python) \(AriesPaths.hookScriptShellPath)"

        let hookEntry: [[String: Any]] = [
            ["type": "command", "command": command],
        ]

        let events: [String] = [
            HookEventName.sessionStart,
            HookEventName.userPromptSubmit,
            HookEventName.preToolUse,
            HookEventName.postToolUse,
            HookEventName.postToolUseFailure,
            HookEventName.stop,
            HookEventName.stopFailure,
            HookEventName.subagentStart,
            HookEventName.subagentStop,
            HookEventName.sessionEnd,
            HookEventName.preCompact,
            HookEventName.postCompact,
        ]

        var hooksConfig: [String: [[String: Any]]] = [:]
        for event in events {
            if HookEventName.wildcardMatcherEvents.contains(event) {
                hooksConfig[event] = [
                    ["if": "*", "hooks": hookEntry],
                ]
            } else if event == HookEventName.preCompact {
                hooksConfig[event] = [
                    ["if": "auto", "hooks": hookEntry],
                    ["if": "manual", "hooks": hookEntry],
                ]
            } else {
                hooksConfig[event] = [
                    ["hooks": hookEntry],
                ]
            }
        }

        let config: [String: Any] = [
            "description": "Aries Island — session state and notification hooks",
            "hooks": hooksConfig,
        ]

        if let data = try? JSONSerialization.data(
            withJSONObject: config,
            options: [.prettyPrinted, .sortedKeys]
        ) {
            try? data.write(to: hooksFile)
        }
    }

    private static func detectPython() -> String {
        let process = Process()
        process.executableURL = URL(fileURLWithPath: "/usr/bin/which")
        process.arguments = ["python3"]
        process.standardOutput = FileHandle.nullDevice
        process.standardError = FileHandle.nullDevice

        do {
            try process.run()
            process.waitUntilExit()
            if process.terminationStatus == 0 {
                return "python3"
            }
        } catch {}

        return "python"
    }
}
