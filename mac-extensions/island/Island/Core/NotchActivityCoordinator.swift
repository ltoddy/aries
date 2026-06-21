import Combine
import SwiftUI

enum NotchActivityType: Equatable {
    case claude
    case none
}

struct ExpandingActivity: Equatable {
    var show: Bool = false
    var type: NotchActivityType = .none
    var value: CGFloat = 0

    static let empty = ExpandingActivity()
}

@MainActor
class NotchActivityCoordinator: ObservableObject {
    static let shared = NotchActivityCoordinator()

    @Published var expandingActivity: ExpandingActivity = .empty {
        didSet {
            if expandingActivity.show {
                scheduleActivityHide()
            } else {
                activityTask?.cancel()
            }
        }
    }

    var activityDuration: TimeInterval = 0

    private var activityTask: Task<Void, Never>?

    private init() {}

    func showActivity(
        type: NotchActivityType,
        value: CGFloat = 0,
        duration: TimeInterval = 0
    ) {
        activityDuration = duration

        withAnimation(.smooth) {
            expandingActivity = ExpandingActivity(
                show: true,
                type: type,
                value: value
            )
        }
    }

    func hideActivity() {
        withAnimation(.smooth) {
            expandingActivity = .empty
        }
    }

    private func scheduleActivityHide() {
        activityTask?.cancel()

        guard activityDuration > 0 else { return }

        let currentType = expandingActivity.type
        activityTask = Task { [weak self] in
            try? await Task.sleep(for: .seconds(self?.activityDuration ?? 3))
            guard let self, !Task.isCancelled else { return }

            if expandingActivity.type == currentType {
                hideActivity()
            }
        }
    }
}
