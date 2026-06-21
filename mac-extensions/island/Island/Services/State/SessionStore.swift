import Combine
import Foundation
import os.log

actor SessionStore {
    static let shared = SessionStore()

    nonisolated static let logger = Logger(subsystem: "com.aries.Island", category: "Session")

    private var sessions: [String: SessionState] = [:]

    private nonisolated(unsafe) let sessionsSubject = CurrentValueSubject<[SessionState], Never>([])

    nonisolated var sessionsPublisher: AnyPublisher<[SessionState], Never> {
        sessionsSubject.eraseToAnyPublisher()
    }

    private init() {}

    func process(_ event: SessionEvent) async {
        Self.logger.debug("Processing: \(String(describing: event), privacy: .public)")

        switch event {
        case let .hookReceived(hookEvent):
            await processHookEvent(hookEvent)

        case let .sessionEnded(sessionId):
            sessions.removeValue(forKey: sessionId)
        }

        publishState()
    }

    private func processHookEvent(_ event: HookEvent) async {
        let sessionId = event.sessionId
        var session = sessions[sessionId] ?? createSession(from: event)

        session.pid = event.pid
        if let tty = event.tty {
            session.tty = tty.replacingOccurrences(of: "/dev/", with: "")
        }
        session.lastActivity = Date()

        if event.status == HookStatus.ended {
            sessions.removeValue(forKey: sessionId)
            return
        }

        let newPhase = event.sessionPhase

        if session.phase.canTransition(to: newPhase) {
            session.phase = newPhase
        } else {
            Self.logger.debug("Invalid transition: \(String(describing: session.phase), privacy: .public) -> \(String(describing: newPhase), privacy: .public), ignoring")
        }

        processToolTracking(event: event, session: &session)
        processSubagentTracking(event: event, session: &session)

        if event.event == HookEventName.userPromptSubmit, let prompt = event.prompt, !prompt.isEmpty {
            let userItem = ChatHistoryItem(
                id: "user-" + UUID().uuidString.prefix(8),
                type: .user(prompt),
                timestamp: Date()
            )
            session.chatItems.append(userItem)

            if session.conversationInfo.firstUserMessage == nil {
                let truncated = prompt.count > 50 ? String(prompt.prefix(47)) + "..." : prompt
                session.conversationInfo = ConversationInfo(
                    summary: nil,
                    lastMessage: session.conversationInfo.lastMessage,
                    lastMessageRole: session.conversationInfo.lastMessageRole,
                    lastToolName: session.conversationInfo.lastToolName,
                    firstUserMessage: truncated,
                    lastUserMessageDate: session.conversationInfo.lastUserMessageDate,
                    usage: session.conversationInfo.usage
                )
            }

            Self.logger.debug("UserPromptSubmit: added user message")
        }

        if event.event == HookEventName.stop || event.event == HookEventName.stopFailure {
            session.subagentState = SubagentState()

            let hasRunning = session.chatItems.contains(where: {
                if case let .toolCall(t) = $0.type, t.status == .running { return true }
                return false
            })
            if hasRunning {
                Self.logger.debug("Stop: completing running tools (PostToolUse not available from rig-core)")
                let newStatus: ToolStatus = event.event == HookEventName.stopFailure ? .error : .success
                for i in 0 ..< session.chatItems.count {
                    if case var .toolCall(tool) = session.chatItems[i].type,
                       tool.status == .running
                    {
                        tool.status = newStatus
                        session.chatItems[i] = ChatHistoryItem(
                            id: session.chatItems[i].id,
                            type: .toolCall(tool),
                            timestamp: session.chatItems[i].timestamp
                        )
                    }
                }
            }

            if let response = event.agentResponse, !response.isEmpty {
                let assistantItem = ChatHistoryItem(
                    id: "assistant-" + UUID().uuidString.prefix(8),
                    type: .assistant(response),
                    timestamp: Date()
                )
                session.chatItems.append(assistantItem)
                Self.logger.debug("Stop: added assistant response")
            }
        }

        sessions[sessionId] = session
    }

    private func createSession(from event: HookEvent) -> SessionState {
        SessionState(
            sessionId: event.sessionId,
            cwd: event.cwd,
            projectName: URL(fileURLWithPath: event.cwd).lastPathComponent,
            pid: event.pid,
            tty: event.tty?.replacingOccurrences(of: "/dev/", with: ""),
            phase: .idle
        )
    }

    private func processToolTracking(event: HookEvent, session: inout SessionState) {
        switch event.event {
        case HookEventName.preToolUse:
            if let toolUseId = event.toolUseId, let toolName = event.tool {
                _ = session.toolTracker.markSeen(toolUseId)

                let isSubagentTool = session.subagentState.hasActiveSubagent && !ToolCallItem.isSubagentContainerName(toolName)
                if isSubagentTool {
                    return
                }

                let toolExists = session.chatItems.contains { $0.id == toolUseId }
                if !toolExists {
                    let input = event.toolInput?.asStringDict ?? [:]

                    let placeholderItem = ChatHistoryItem(
                        id: toolUseId,
                        type: .toolCall(ToolCallItem(
                            name: toolName,
                            input: input,
                            status: .running,
                            result: nil,
                            structuredResult: nil,
                            subagentTools: []
                        )),
                        timestamp: Date()
                    )
                    session.chatItems.append(placeholderItem)
                    Self.logger.debug("Created placeholder tool entry for \(toolUseId.prefix(16), privacy: .public)")
                }
            }

        case HookEventName.postToolUse:
            if let toolUseId = event.toolUseId {
                Self.logger.debug("PostToolUse: id=\(toolUseId.prefix(16), privacy: .public), tool=\(event.tool ?? "?")")
                var found = false
                for i in 0 ..< session.chatItems.count {
                    if session.chatItems[i].id == toolUseId,
                       case var .toolCall(tool) = session.chatItems[i].type,
                       tool.status == .running
                    {
                        tool.status = .success
                        session.chatItems[i] = ChatHistoryItem(
                            id: toolUseId,
                            type: .toolCall(tool),
                            timestamp: session.chatItems[i].timestamp
                        )
                        found = true
                        Self.logger.debug("PostToolUse: matched by id, marked success")
                        break
                    }
                }

                if !found, let toolName = event.tool {
                    Self.logger.debug("PostToolUse: id match failed, trying by name=\(toolName)")
                    for i in 0 ..< session.chatItems.count {
                        if case var .toolCall(tool) = session.chatItems[i].type,
                           tool.name == toolName,
                           tool.status == .running
                        {
                            tool.status = .success
                            session.chatItems[i] = ChatHistoryItem(
                                id: session.chatItems[i].id,
                                type: .toolCall(tool),
                                timestamp: session.chatItems[i].timestamp
                            )
                            Self.logger.debug("PostToolUse: matched by name, marked success")
                            break
                        }
                    }
                }
            }

        case HookEventName.postToolUseFailure:
            if let toolUseId = event.toolUseId {
                Self.logger.debug("PostToolUseFailure: id=\(toolUseId.prefix(16), privacy: .public), tool=\(event.tool ?? "?")")
                var found = false
                for i in 0 ..< session.chatItems.count {
                    if session.chatItems[i].id == toolUseId,
                       case var .toolCall(tool) = session.chatItems[i].type,
                       tool.status == .running
                    {
                        tool.status = .error
                        session.chatItems[i] = ChatHistoryItem(
                            id: toolUseId,
                            type: .toolCall(tool),
                            timestamp: session.chatItems[i].timestamp
                        )
                        found = true
                        Self.logger.debug("PostToolUseFailure: matched by id, marked error")
                        break
                    }
                }

                if !found, let toolName = event.tool {
                    Self.logger.debug("PostToolUseFailure: id match failed, trying by name=\(toolName)")
                    for i in 0 ..< session.chatItems.count {
                        if case var .toolCall(tool) = session.chatItems[i].type,
                           tool.name == toolName,
                           tool.status == .running
                        {
                            tool.status = .error
                            session.chatItems[i] = ChatHistoryItem(
                                id: session.chatItems[i].id,
                                type: .toolCall(tool),
                                timestamp: session.chatItems[i].timestamp
                            )
                            break
                        }
                    }
                }
            }

        default:
            break
        }
    }

    private func processSubagentTracking(event: HookEvent, session: inout SessionState) {
        switch event.event {
        case HookEventName.preToolUse:
            if ToolCallItem.isSubagentContainerName(event.tool), let toolUseId = event.toolUseId {
                let description = event.toolInput?.stringValue(for: "description")
                session.subagentState.startTask(taskToolId: toolUseId, description: description)
                Self.logger.debug("Started Task/Agent subagent tracking: \(toolUseId.prefix(12), privacy: .public)")
            } else if let toolName = event.tool,
                      let toolUseId = event.toolUseId,
                      session.subagentState.hasActiveSubagent
            {
                let input = event.toolInput?.asStringDict ?? [:]
                let subagentTool = SubagentToolCall(
                    id: toolUseId,
                    name: toolName,
                    input: input,
                    status: .running,
                    timestamp: Date()
                )
                session.subagentState.addSubagentTool(subagentTool)
                syncSubagentToolsToChatItems(session: &session)
            }

        case HookEventName.postToolUse:
            if ToolCallItem.isSubagentContainerName(event.tool), let toolUseId = event.toolUseId {
                session.subagentState.stopTask(taskToolId: toolUseId)
                Self.logger.debug("Stopped subagent tracking for \(toolUseId.prefix(12), privacy: .public)")
            } else if let toolUseId = event.toolUseId,
                      session.subagentState.hasActiveSubagent
            {
                session.subagentState.updateSubagentToolStatus(toolId: toolUseId, status: .success)
                syncSubagentToolsToChatItems(session: &session)
            }

        case HookEventName.postToolUseFailure:
            if let toolUseId = event.toolUseId,
               session.subagentState.hasActiveSubagent
            {
                session.subagentState.updateSubagentToolStatus(toolId: toolUseId, status: .error)
                syncSubagentToolsToChatItems(session: &session)
            }

        case HookEventName.subagentStop:
            Self.logger.debug("SubagentStop received")

        default:
            break
        }
    }

    private func syncSubagentToolsToChatItems(session: inout SessionState) {
        for (taskToolId, context) in session.subagentState.activeTasks {
            guard !context.subagentTools.isEmpty else { continue }
            for i in 0 ..< session.chatItems.count {
                if session.chatItems[i].id == taskToolId,
                   case var .toolCall(tool) = session.chatItems[i].type
                {
                    tool.subagentTools = context.subagentTools
                    session.chatItems[i] = ChatHistoryItem(
                        id: taskToolId,
                        type: .toolCall(tool),
                        timestamp: session.chatItems[i].timestamp
                    )
                    break
                }
            }
        }
    }

    private func publishState() {
        let sortedSessions = Array(sessions.values).sorted { $0.projectName < $1.projectName }
        sessionsSubject.send(sortedSessions)
    }
}
