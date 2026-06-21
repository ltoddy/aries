import Foundation

enum SessionEvent {
    case hookReceived(HookEvent)
    case sessionEnded(sessionId: String)
}

extension SessionEvent: CustomStringConvertible {
    nonisolated var description: String {
        switch self {
        case let .hookReceived(event):
            "hookReceived(\(event.event), session: \(event.sessionId.prefix(8)))"
        case let .sessionEnded(sessionId):
            "sessionEnded(session: \(sessionId.prefix(8)))"
        }
    }
}
