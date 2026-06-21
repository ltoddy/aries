import Combine
import SwiftUI

struct FerrisIcon: View {
    let size: CGFloat
    let color: Color
    var animateLegs: Bool = false

    @State private var bounce = false

    init(size: CGFloat = 16, color: Color = Color(red: 0.89, green: 0.22, blue: 0.09), animateLegs: Bool = false) {
        self.size = size
        self.color = color
        self.animateLegs = animateLegs
    }

    var body: some View {
        Image("Ferris")
            .resizable()
            .interpolation(.high)
            .aspectRatio(contentMode: .fit)
            .frame(width: size, height: size)
            .scaleEffect(bounce ? 1.08 : 1.0)
            .animation(
                animateLegs
                    ? .easeInOut(duration: 0.45).repeatForever(autoreverses: true)
                    : .default,
                value: bounce
            )
            .onAppear { bounce = animateLegs }
            .onChange(of: animateLegs) { _, newValue in bounce = newValue }
    }
}

struct ReadyForInputIndicatorIcon: View {
    let size: CGFloat
    let color: Color

    init(size: CGFloat = 14, color: Color = TerminalColors.green) {
        self.size = size
        self.color = color
    }

    private let pixels: [(CGFloat, CGFloat)] = [
        (5, 15),
        (9, 19),
        (13, 23),
        (17, 19),
        (21, 15),
        (25, 11),
        (29, 7),
    ]

    var body: some View {
        Canvas { context, _ in
            let scale = size / 30.0
            let pixelSize: CGFloat = 4 * scale

            for (x, y) in pixels {
                let rect = CGRect(
                    x: x * scale - pixelSize / 2,
                    y: y * scale - pixelSize / 2,
                    width: pixelSize,
                    height: pixelSize
                )
                context.fill(Path(rect), with: .color(color))
            }
        }
        .frame(width: size, height: size)
    }
}
