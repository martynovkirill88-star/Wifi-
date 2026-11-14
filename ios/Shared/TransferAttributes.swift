import ActivityKit

struct TransferAttributes: ActivityAttributes {
    struct ContentState: Codable, Hashable {
        var message: String
        var done: Int64
        var total: Int64
        var finished: Bool
    }
    var computer: String
}
