import Combine
import Foundation

@MainActor
class ChatHistoryManager: ObservableObject {
    static let shared = ChatHistoryManager()

    @Published private(set) var histories: [String: [ChatHistoryItem]] = [:]
    @Published private(set) var agentDescriptions: [String: [String: String]] = [:]

    private var cancellables = Set<AnyCancellable>()

    private init() {
        SessionStore.shared.sessionsPublisher
            .receive(on: DispatchQueue.main)
            .sink { [weak self] sessions in
                self?.updateFromSessions(sessions)
            }
            .store(in: &cancellables)
    }

    func history(for sessionId: String) -> [ChatHistoryItem] {
        histories[sessionId] ?? []
    }

    private func updateFromSessions(_ sessions: [SessionState]) {
        var newHistories: [String: [ChatHistoryItem]] = [:]
        var newAgentDescriptions: [String: [String: String]] = [:]
        for session in sessions {
            let filteredItems = filterOutSubagentTools(session.chatItems)
            newHistories[session.sessionId] = filteredItems
            newAgentDescriptions[session.sessionId] = session.subagentState.agentDescriptions
        }
        histories = newHistories
        agentDescriptions = newAgentDescriptions
    }

    private func filterOutSubagentTools(_ items: [ChatHistoryItem]) -> [ChatHistoryItem] {
        var subagentToolIds = Set<String>()
        for item in items {
            if case let .toolCall(tool) = item.type, tool.isSubagentContainer {
                for subagentTool in tool.subagentTools {
                    subagentToolIds.insert(subagentTool.id)
                }
            }
        }

        return items.filter { !subagentToolIds.contains($0.id) }
    }
}

struct ChatHistoryItem: Identifiable, Equatable {
    let id: String
    let type: ChatHistoryItemType
    let timestamp: Date

    static func == (lhs: ChatHistoryItem, rhs: ChatHistoryItem) -> Bool {
        lhs.id == rhs.id && lhs.type == rhs.type
    }
}

enum ChatHistoryItemType: Equatable {
    case user(String)
    case assistant(String)
    case toolCall(ToolCallItem)
    case thinking(String)
    case image(ImageBlock)
    case interrupted
}

struct ToolCallItem: Equatable {
    let name: String
    let input: [String: String]
    var status: ToolStatus
    var result: String?
    var structuredResult: ToolResultData?

    var subagentTools: [SubagentToolCall]

    var isSubagentContainer: Bool {
        Self.isSubagentContainerName(name)
    }

    static func isSubagentContainerName(_ name: String?) -> Bool {
        guard let name else { return false }
        return ToolName.subagentContainerNames.contains(name)
    }

    var statusDisplay: ToolStatusDisplay {
        if status == .running {
            return ToolStatusDisplay.running(for: name, input: input)
        }
        if status == .interrupted {
            return ToolStatusDisplay(text: "Interrupted")
        }
        return ToolStatusDisplay.completed(for: name, result: structuredResult)
    }
}

/// Well-known tool name constants used by the Aries agent SDK.
enum ToolName {
    nonisolated static let task = "Task"
    nonisolated static let agent = "Agent"
    nonisolated static let edit = "Edit"
    nonisolated static let agentOutputTool = "AgentOutputTool"

    nonisolated static let subagentContainerNames: Set<String> = [task, agent]
}

enum ToolStatus: CustomStringConvertible {
    case running
    case success
    case error
    case interrupted

    nonisolated var description: String {
        switch self {
        case .running: "running"
        case .success: "success"
        case .error: "error"
        case .interrupted: "interrupted"
        }
    }
}

extension ToolStatus: Equatable {
    nonisolated static func == (lhs: ToolStatus, rhs: ToolStatus) -> Bool {
        switch (lhs, rhs) {
        case (.running, .running): true
        case (.success, .success): true
        case (.error, .error): true
        case (.interrupted, .interrupted): true
        default: false
        }
    }
}

struct SubagentToolCall: Equatable, Identifiable {
    let id: String
    let name: String
    let input: [String: String]
    var status: ToolStatus
    let timestamp: Date
}
