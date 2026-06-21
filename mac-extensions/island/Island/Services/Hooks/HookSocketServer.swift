import Foundation
import os.log

private let logger = Logger(subsystem: "com.aries.Island", category: "Hooks")

// MARK: - Hook Event Name Constants

/// Hook event names matching the Rust `HOOK_EVENT_NAME` constants in `crates/aries-extension/src/hook/input/`.
enum HookEventName {
    nonisolated static let sessionStart = "SessionStart"
    nonisolated static let sessionEnd = "SessionEnd"
    nonisolated static let userPromptSubmit = "UserPromptSubmit"
    nonisolated static let preToolUse = "PreToolUse"
    nonisolated static let postToolUse = "PostToolUse"
    nonisolated static let postToolUseFailure = "PostToolUseFailure"
    nonisolated static let stop = "Stop"
    nonisolated static let stopFailure = "StopFailure"
    nonisolated static let subagentStart = "SubagentStart"
    nonisolated static let subagentStop = "SubagentStop"
    nonisolated static let preCompact = "PreCompact"
    nonisolated static let postCompact = "PostCompact"

    /// Events that accept a wildcard matcher (i.e. trigger on any tool).
    nonisolated static let wildcardMatcherEvents: Set<String> = [
        preToolUse, postToolUse, postToolUseFailure,
    ]
}

// MARK: - Hook Status Constants

/// Status strings produced by the hook bridge layer.
enum HookStatus {
    nonisolated static let waitingForInput = "waiting_for_input"
    nonisolated static let runningTool = "running_tool"
    nonisolated static let processing = "processing"
    nonisolated static let starting = "starting"
    nonisolated static let compacting = "compacting"
    nonisolated static let ended = "ended"
}

// MARK: - Tool Input Model

/// Typed model for tool input fields extracted from hook events.
/// Replaces raw `[String: AnyCodable]` dictionary access with a proper model.
struct ToolInputModel: Codable {
    let raw: [String: AnyCodable]

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        raw = try container.decode([String: AnyCodable].self)
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()
        try container.encode(raw)
    }

    /// Converts the raw dictionary to `[String: String]` for display purposes.
    var asStringDict: [String: String] {
        var result: [String: String] = [:]
        for (key, value) in raw {
            if let str = value.value as? String {
                result[key] = str
            } else if let num = value.value as? Int {
                result[key] = String(num)
            } else if let bool = value.value as? Bool {
                result[key] = bool ? "true" : "false"
            }
        }
        return result
    }

    func stringValue(for key: String) -> String? {
        guard let codable = raw[key] else { return nil }
        if let str = codable.value as? String { return str }
        if let num = codable.value as? Int { return String(num) }
        if let bool = codable.value as? Bool { return bool ? "true" : "false" }
        return nil
    }
}

// MARK: - Hook Event

struct HookEvent: Codable {
    let sessionId: String
    let cwd: String
    let event: String
    let status: String
    let pid: Int?
    let tty: String?
    let tool: String?
    let toolInput: ToolInputModel?
    let toolUseId: String?
    let prompt: String?
    /// Maps to `tool_response` / `last_assistant_message` depending on the event type
    /// (see Rust `PostToolUseHookInput.tool_response`, `StopHookInput.last_assistant_message`).
    let agentResponse: String?

    enum CodingKeys: String, CodingKey {
        case sessionId = "session_id"
        case cwd, event, status, pid, tty, tool
        case toolInput = "tool_input"
        case toolUseId = "tool_use_id"
        case prompt
        case agentResponse = "agent_response"
    }

    var sessionPhase: SessionPhase {
        if event == HookEventName.preCompact {
            return .compacting
        }

        switch status {
        case HookStatus.waitingForInput:
            return .waitingForInput
        case HookStatus.runningTool, HookStatus.processing, HookStatus.starting:
            return .processing
        case HookStatus.compacting:
            return .compacting
        default:
            return .idle
        }
    }
}

typealias HookEventHandler = @Sendable (HookEvent) -> Void

class HookSocketServer {
    static let shared = HookSocketServer()
    static let socketPath = "/tmp/aries-island.sock"

    private var serverSocket: Int32 = -1
    private var acceptSource: DispatchSourceRead?
    private var eventHandler: HookEventHandler?
    private let queue = DispatchQueue(label: "com.aries.Island.socket", qos: .userInitiated)

    private init() {}

    func start(onEvent: @escaping HookEventHandler) {
        queue.async { [weak self] in
            self?.startServer(onEvent: onEvent)
        }
    }

    private func startServer(onEvent: @escaping HookEventHandler) {
        guard serverSocket < 0 else { return }

        eventHandler = onEvent

        unlink(Self.socketPath)

        serverSocket = socket(AF_UNIX, SOCK_STREAM, 0)
        guard serverSocket >= 0 else {
            logger.error("Failed to create socket: \(errno)")
            return
        }

        let flags = fcntl(serverSocket, F_GETFL)
        _ = fcntl(serverSocket, F_SETFL, flags | O_NONBLOCK)

        var addr = sockaddr_un()
        addr.sun_family = sa_family_t(AF_UNIX)
        Self.socketPath.withCString { ptr in
            withUnsafeMutablePointer(to: &addr.sun_path) { pathPtr in
                let pathBufferPtr = UnsafeMutableRawPointer(pathPtr)
                    .assumingMemoryBound(to: CChar.self)
                strcpy(pathBufferPtr, ptr)
            }
        }

        let bindResult = withUnsafePointer(to: &addr) { ptr in
            ptr.withMemoryRebound(to: sockaddr.self, capacity: 1) { sockaddrPtr in
                bind(serverSocket, sockaddrPtr, socklen_t(MemoryLayout<sockaddr_un>.size))
            }
        }

        guard bindResult == 0 else {
            logger.error("Failed to bind socket: \(errno)")
            close(serverSocket)
            serverSocket = -1
            return
        }

        chmod(Self.socketPath, 0o600)

        guard listen(serverSocket, 10) == 0 else {
            logger.error("Failed to listen: \(errno)")
            close(serverSocket)
            serverSocket = -1
            return
        }

        logger.info("Listening on \(Self.socketPath, privacy: .public)")

        acceptSource = DispatchSource.makeReadSource(fileDescriptor: serverSocket, queue: queue)
        acceptSource?.setEventHandler { [weak self] in
            self?.acceptConnection()
        }
        acceptSource?.setCancelHandler { [weak self] in
            if let fd = self?.serverSocket, fd >= 0 {
                close(fd)
                self?.serverSocket = -1
            }
        }
        acceptSource?.resume()
    }

    func stop() {
        acceptSource?.cancel()
        acceptSource = nil
        unlink(Self.socketPath)
    }

    private func acceptConnection() {
        let clientSocket = accept(serverSocket, nil, nil)
        guard clientSocket >= 0 else { return }

        var nosigpipe: Int32 = 1
        setsockopt(clientSocket, SOL_SOCKET, SO_NOSIGPIPE, &nosigpipe, socklen_t(MemoryLayout<Int32>.size))

        handleClient(clientSocket)
    }

    private func handleClient(_ clientSocket: Int32) {
        let flags = fcntl(clientSocket, F_GETFL)
        _ = fcntl(clientSocket, F_SETFL, flags | O_NONBLOCK)

        var allData = Data()
        var buffer = [UInt8](repeating: 0, count: 131_072)
        var pollFd = pollfd(fd: clientSocket, events: Int16(POLLIN), revents: 0)

        let startTime = Date()
        while Date().timeIntervalSince(startTime) < 0.5 {
            let pollResult = poll(&pollFd, 1, 50)

            if pollResult > 0, (pollFd.revents & Int16(POLLIN)) != 0 {
                let bytesRead = read(clientSocket, &buffer, buffer.count)

                if bytesRead > 0 {
                    allData.append(contentsOf: buffer[0 ..< bytesRead])
                } else if bytesRead == 0 {
                    break
                } else if errno != EAGAIN, errno != EWOULDBLOCK {
                    break
                }
            } else if pollResult == 0 {
                if !allData.isEmpty {
                    break
                }
            } else {
                break
            }
        }

        close(clientSocket)

        guard !allData.isEmpty else {
            return
        }

        let data = allData

        guard let event = try? JSONDecoder().decode(HookEvent.self, from: data) else {
            logger.warning("Failed to parse event: \(String(data: data, encoding: .utf8) ?? "?", privacy: .public)")
            return
        }

        logger.debug("Received: \(event.event, privacy: .public) for \(event.sessionId.prefix(8), privacy: .public) tool=\(event.tool ?? "nil", privacy: .public) toolUseId=\(event.toolUseId?.prefix(12) ?? "nil", privacy: .public)")

        eventHandler?(event)
    }
}

struct AnyCodable: Codable, @unchecked Sendable {
    nonisolated(unsafe) let value: Any

    init(_ value: Any) {
        self.value = value
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()

        if container.decodeNil() {
            value = NSNull()
        } else if let bool = try? container.decode(Bool.self) {
            value = bool
        } else if let int = try? container.decode(Int.self) {
            value = int
        } else if let double = try? container.decode(Double.self) {
            value = double
        } else if let string = try? container.decode(String.self) {
            value = string
        } else if let array = try? container.decode([AnyCodable].self) {
            value = array.map(\.value)
        } else if let dict = try? container.decode([String: AnyCodable].self) {
            value = dict.mapValues { $0.value }
        } else {
            throw DecodingError.dataCorruptedError(in: container, debugDescription: "Cannot decode value")
        }
    }

    func encode(to encoder: Encoder) throws {
        var container = encoder.singleValueContainer()

        switch value {
        case is NSNull:
            try container.encodeNil()
        case let bool as Bool:
            try container.encode(bool)
        case let int as Int:
            try container.encode(int)
        case let double as Double:
            try container.encode(double)
        case let string as String:
            try container.encode(string)
        case let array as [Any]:
            try container.encode(array.map { AnyCodable($0) })
        case let dict as [String: Any]:
            try container.encode(dict.mapValues { AnyCodable($0) })
        default:
            throw EncodingError.invalidValue(value, EncodingError.Context(codingPath: [], debugDescription: "Cannot encode value"))
        }
    }
}
