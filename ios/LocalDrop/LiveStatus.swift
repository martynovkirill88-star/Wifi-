import ActivityKit
import UIKit

@MainActor final class LiveStatus {
    private var activity: Activity<TransferAttributes>?
    private var lastUpdate = Date.distantPast
    init() {
        // Reconcile banners left behind by a force quit; never show them as running.
        for old in Activity<TransferAttributes>.activities {
            Task { await old.end(ActivityContent(state:.init(message:"Приложение было закрыто. Откройте LocalDrop для продолжения.",done:0,total:0,finished:true),staleDate:nil),dismissalPolicy:.immediate) }
        }
    }
    func start(computer:String,message:String) {
        guard activity == nil, ActivityAuthorizationInfo().areActivitiesEnabled, UIApplication.shared.applicationState == .active else { return }
        do {
            activity = try Activity.request(attributes:TransferAttributes(computer:computer.isEmpty ? "Windows" : computer),content:ActivityContent(state:.init(message:message,done:0,total:0,finished:false),staleDate:Date().addingTimeInterval(60)),pushType:nil)
            lastUpdate = .distantPast
        } catch { /* Transfer remains available when the system disallows Live Activities. */ }
    }
    func update(message:String,done:Int64,total:Int64) {
        guard let activity, Date().timeIntervalSince(lastUpdate) >= 1 else { return }
        lastUpdate = Date()
        Task { await activity.update(ActivityContent(state:.init(message:message,done:done,total:total,finished:false),staleDate:Date().addingTimeInterval(60))) }
    }
    func end(message:String,done:Int64,total:Int64) {
        guard let old = activity else { return }; activity = nil
        Task { await old.end(ActivityContent(state:.init(message:message,done:done,total:total,finished:true),staleDate:nil),dismissalPolicy:.after(Date().addingTimeInterval(60))) }
    }
}
