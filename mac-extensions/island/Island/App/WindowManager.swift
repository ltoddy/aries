import AppKit
import os.log

private let logger = Logger(subsystem: "com.aries.Island", category: "Window")

class WindowManager {
    private(set) var windowController: NotchWindowController?
    private var isInitialLaunch = true
    private var currentScreenFrame: NSRect?

    func setupNotchWindow() -> NotchWindowController? {
        guard let screen = NSScreen.builtin ?? NSScreen.main else {
            logger.warning("No screen found")
            return nil
        }

        if let existingController = windowController,
           let existingFrame = currentScreenFrame,
           existingFrame == screen.frame
        {
            logger.debug("Screen unchanged, skipping window recreation")
            return existingController
        }

        let shouldAnimate = isInitialLaunch
        isInitialLaunch = false

        if let existingController = windowController {
            existingController.window?.orderOut(nil)
            existingController.window?.close()
            windowController = nil
        }

        currentScreenFrame = screen.frame
        windowController = NotchWindowController(screen: screen, animateOnLaunch: shouldAnimate)
        windowController?.showWindow(nil)

        return windowController
    }
}
