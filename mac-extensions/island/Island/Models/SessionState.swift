import Foundation

struct UsageInfo: Equatable {
    var inputTokens: Int = 0
    var outputTokens: Int = 0
    var cacheReadTokens: Int = 0
    var cacheCreationTokens: Int = 0

    var totalTokens: Int {
        inputTokens + outputTokens
    }

    var formattedTotal: String {
        let total = totalTokens
        if total >= 1_000_000 {
            return String(format: "%.1fM", Double(total) / 1_000_000)
        } else if total >= 1000 {
            return String(format: "%.1fK", Double(total) / 1000)
        }
        return "\(total)"
    }
}

struct ConversationInfo: Equatable {
    let summary: String?
    let lastMessage: String?
    let lastMessageRole: String?
    let lastToolName: String?
    let firstUserMessage: String?
    let lastUserMessageDate: Date?
    var usage: UsageInfo = .init()
}

struct SessionState: Equatable, Identifiable {
    let sessionId: String
    let cwd: String
    let projectName: String

    var pid: Int?
    var tty: String?

    var phase: SessionPhase

    var chatItems: [ChatHistoryItem]

    var toolTracker: ToolTracker

    var subagentState: SubagentState

    var conversationInfo: ConversationInfo

    var lastActivity: Date
    var createdAt: Date

    var id: String {
        sessionId
    }

    nonisolated init(
        sessionId: String,
        cwd: String,
        projectName: String? = nil,
        pid: Int? = nil,
        tty: String? = nil,
        phase: SessionPhase = .idle,
        chatItems: [ChatHistoryItem] = [],
        toolTracker: ToolTracker = ToolTracker(),
        subagentState: SubagentState = SubagentState(),
        conversationInfo: ConversationInfo = ConversationInfo(
            summary: nil, lastMessage: nil, lastMessageRole: nil,
            lastToolName: nil, firstUserMessage: nil, lastUserMessageDate: nil
        ),
        lastActivity: Date = Date(),
        createdAt: Date = Date()
    ) {
        self.sessionId = sessionId
        self.cwd = cwd
        self.projectName = projectName ?? URL(fileURLWithPath: cwd).lastPathComponent
        self.pid = pid
        self.tty = tty
        self.phase = phase
        self.chatItems = chatItems
        self.toolTracker = toolTracker
        self.subagentState = subagentState
        self.conversationInfo = conversationInfo
        self.lastActivity = lastActivity
        self.createdAt = createdAt
    }

    var needsAttention: Bool {
        phase.needsAttention
    }

    var stableId: String {
        if let pid {
            return "\(pid)-\(sessionId)"
        }
        return sessionId
    }

    var displayTitle: String {
        conversationInfo.summary ?? conversationInfo.firstUserMessage ?? projectName
    }

    var lastMessage: String? {
        conversationInfo.lastMessage
    }

    var lastMessageRole: String? {
        conversationInfo.lastMessageRole
    }

    var lastToolName: String? {
        conversationInfo.lastToolName
    }

    var lastUserMessageDate: Date? {
        conversationInfo.lastUserMessageDate
    }

    var usage: UsageInfo {
        conversationInfo.usage
    }
}

struct ToolTracker: Equatable {
    var seenIds: Set<String>

    nonisolated init(
        seenIds: Set<String> = []
    ) {
        self.seenIds = seenIds
    }

    nonisolated mutating func markSeen(_ id: String) -> Bool {
        seenIds.insert(id).inserted
    }
}

struct SubagentState: Equatable {
    var activeTasks: [String: TaskContext]

    var taskStack: [String]

    var agentDescriptions: [String: String]

    nonisolated init(activeTasks: [String: TaskContext] = [:], taskStack: [String] = [], agentDescriptions: [String: String] = [:]) {
        self.activeTasks = activeTasks
        self.taskStack = taskStack
        self.agentDescriptions = agentDescriptions
    }

    nonisolated var hasActiveSubagent: Bool {
        !activeTasks.isEmpty
    }

    nonisolated mutating func startTask(taskToolId: String, description: String? = nil) {
        activeTasks[taskToolId] = TaskContext(
            taskToolId: taskToolId,
            startTime: Date(),
            agentId: nil,
            description: description,
            subagentTools: []
        )
    }

    nonisolated mutating func stopTask(taskToolId: String) {
        activeTasks.removeValue(forKey: taskToolId)
    }

    nonisolated mutating func addSubagentTool(_ tool: SubagentToolCall) {
        guard let mostRecentTaskId = activeTasks.keys.max(by: {
            (activeTasks[$0]?.startTime ?? .distantPast) < (activeTasks[$1]?.startTime ?? .distantPast)
        }) else { return }

        activeTasks[mostRecentTaskId]?.subagentTools.append(tool)
    }

    nonisolated mutating func updateSubagentToolStatus(toolId: String, status: ToolStatus) {
        for taskId in activeTasks.keys {
            if let index = activeTasks[taskId]?.subagentTools.firstIndex(where: { $0.id == toolId }) {
                activeTasks[taskId]?.subagentTools[index].status = status
                return
            }
        }
    }
}

struct TaskContext: Equatable {
    let taskToolId: String
    let startTime: Date
    var agentId: String?
    var description: String?
    var subagentTools: [SubagentToolCall]
}
