import Foundation

enum SessionPhase {
    case idle

    case processing

    case waitingForInput

    case compacting

    case ended

    nonisolated func canTransition(to next: SessionPhase) -> Bool {
        switch (self, next) {
        case (.ended, _):
            false
        case (_, .ended):
            true
        case (.idle, .processing):
            true
        case (.idle, .compacting):
            true
        case (.processing, .waitingForInput):
            true
        case (.processing, .compacting):
            true
        case (.processing, .idle):
            true
        case (.waitingForInput, .processing):
            true
        case (.waitingForInput, .idle):
            true
        case (.waitingForInput, .compacting):
            true
        case (.compacting, .processing):
            true
        case (.compacting, .idle):
            true
        case (.compacting, .waitingForInput):
            true
        default:
            self == next
        }
    }

    var needsAttention: Bool {
        switch self {
        case .waitingForInput:
            true
        default:
            false
        }
    }
}

extension SessionPhase: Equatable {}

extension SessionPhase: CustomStringConvertible {
    nonisolated var description: String {
        switch self {
        case .idle:
            "idle"
        case .processing:
            "processing"
        case .waitingForInput:
            "waitingForInput"
        case .compacting:
            "compacting"
        case .ended:
            "ended"
        }
    }
}
