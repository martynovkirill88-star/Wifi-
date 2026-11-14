import SwiftUI
import WidgetKit
import ActivityKit

@main struct TransferWidgetBundle: WidgetBundle {
    var body: some Widget { TransferWidget() }
}
struct TransferWidget: Widget {
    var body: some WidgetConfiguration {
        ActivityConfiguration(for:TransferAttributes.self) { context in
            VStack(alignment:.leading,spacing:8) {
                Label("LocalDrop · \(context.attributes.computer)",systemImage:"arrow.left.arrow.right").font(.headline)
                Text(context.isStale && !context.state.finished ? "Обновление приостановлено — откройте LocalDrop" : context.state.message).font(.subheadline).lineLimit(2)
                if context.state.total > 0 { ProgressView(value:Double(context.state.done),total:Double(max(context.state.total,1))) }
                Text("\(ByteCountFormatter.string(fromByteCount:context.state.done,countStyle:.file)) / \(ByteCountFormatter.string(fromByteCount:context.state.total,countStyle:.file))").font(.caption.monospacedDigit())
            }.padding().activityBackgroundTint(Color.black.opacity(0.85)).activitySystemActionForegroundColor(.white).foregroundStyle(.white)
        } dynamicIsland: { context in
            DynamicIsland {
                DynamicIslandExpandedRegion(.leading) { Image(systemName:"arrow.left.arrow.right") }
                DynamicIslandExpandedRegion(.trailing) { Text("LocalDrop").font(.caption) }
                DynamicIslandExpandedRegion(.bottom) {
                    VStack {
                        Text(context.isStale ? "Откройте приложение" : context.state.message).font(.caption).lineLimit(1)
                        if context.state.total > 0 { ProgressView(value:Double(context.state.done),total:Double(max(context.state.total,1))) }
                    }
                }
            } compactLeading: { Image(systemName:"arrow.left.arrow.right") }
              compactTrailing: { Text(percent(context.state)).font(.caption2) }
              minimal: { Image(systemName:"arrow.left.arrow.right") }
        }
    }
    private func percent(_ s: TransferAttributes.ContentState) -> String { s.total > 0 ? "\(Int(min(100,Double(s.done)/Double(s.total)*100)))%" : "…" }
}
