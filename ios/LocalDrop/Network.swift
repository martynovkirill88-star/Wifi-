import Foundation
import CryptoKit
import Security

struct Invitation: Codable {
    let v: Int
    let host: String
    let port: Int
    let fingerprint: String
    let token: String
    func validate() throws {
        guard v == 1, port == 45831, token.count == 64, fingerprint.count == 64,
              token.allSatisfy({ $0.isHexDigit }), fingerprint.allSatisfy({ $0.isHexDigit }),
              host.split(separator: ".").count == 4,
              host.split(separator: ".").allSatisfy({ UInt8($0) != nil }) else { throw TransferError.message("Некорректный код подключения") }
    }
}
struct Envelope: Codable { let id: String; let name: String; let size: UInt64; let sha256: String; let text: String? }
enum TransferError: LocalizedError {
    case message(String)
    var errorDescription: String? { if case let .message(s) = self { return s }; return nil }
}
final class Transport: NSObject, URLSessionDelegate, URLSessionTaskDelegate, URLSessionDownloadDelegate {
    let invitation: Invitation
    var token: String?
    var progress: ((Int64, Int64) -> Void)?
    private var lastProgress = Date.distantPast
    lazy var session: URLSession = {
        let c = URLSessionConfiguration.ephemeral
        c.timeoutIntervalForRequest = 70
        c.timeoutIntervalForResource = 24 * 3600
        c.httpMaximumConnectionsPerHost = 3
        c.urlCache = nil
        c.httpCookieStorage = nil
        return URLSession(configuration: c, delegate: self, delegateQueue: nil)
    }()
    init(_ invitation: Invitation) { self.invitation = invitation }
    func close() { session.invalidateAndCancel() }
    func urlSession(_ session: URLSession, didReceive challenge: URLAuthenticationChallenge,
                    completionHandler: @escaping (URLSession.AuthChallengeDisposition, URLCredential?) -> Void) {
        guard challenge.protectionSpace.authenticationMethod == NSURLAuthenticationMethodServerTrust,
              challenge.protectionSpace.host == invitation.host,
              let trust = challenge.protectionSpace.serverTrust,
              let cert = SecTrustGetCertificateAtIndex(trust, 0) else { completionHandler(.cancelAuthenticationChallenge, nil); return }
        let hash = SHA256.hash(data: SecCertificateCopyData(cert) as Data).map { String(format: "%02x", $0) }.joined()
        guard hash == invitation.fingerprint.lowercased() else { completionHandler(.cancelAuthenticationChallenge, nil); return }
        completionHandler(.useCredential, URLCredential(trust: trust))
    }
    func urlSession(_ session: URLSession, task: URLSessionTask, willPerformHTTPRedirection response: HTTPURLResponse,
                    newRequest request: URLRequest, completionHandler: @escaping (URLRequest?) -> Void) { completionHandler(nil) }
    func report(_ done: Int64, _ total: Int64) {
        if Date().timeIntervalSince(lastProgress) >= 0.2 || done == total { lastProgress = Date(); progress?(done, total) }
    }
    func urlSession(_ session: URLSession, task: URLSessionTask, didSendBodyData bytesSent: Int64, totalBytesSent: Int64, totalBytesExpectedToSend: Int64) { report(totalBytesSent, totalBytesExpectedToSend) }
    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didWriteData bytesWritten: Int64, totalBytesWritten: Int64, totalBytesExpectedToWrite: Int64) { report(totalBytesWritten, totalBytesExpectedToWrite) }
    func urlSession(_ session: URLSession, downloadTask: URLSessionDownloadTask, didFinishDownloadingTo location: URL) {}
    func request(_ path: String, method: String = "GET", query: [URLQueryItem] = []) throws -> URLRequest {
        var c = URLComponents(); c.scheme = "https"; c.host = invitation.host; c.port = invitation.port; c.path = "/v1/" + path; c.queryItems = query.isEmpty ? nil : query
        guard let url = c.url else { throw TransferError.message("Неверный адрес") }
        var r = URLRequest(url: url); r.httpMethod = method
        if let token { r.setValue("Bearer \(token)", forHTTPHeaderField: "Authorization") }
        return r
    }
    func check(_ response: URLResponse) throws -> Int {
        guard let r = response as? HTTPURLResponse else { throw TransferError.message("Нет ответа компьютера") }
        guard (200..<300).contains(r.statusCode) else { throw TransferError.message("Компьютер вернул HTTP \(r.statusCode). Проверьте сопряжение, свободное место и очередь передач.") }
        return r.statusCode
    }
    func json(_ path: String, object: [String:String]) async throws -> Data {
        var r = try request(path, method: "POST"); r.httpBody = try JSONSerialization.data(withJSONObject: object); r.setValue("application/json", forHTTPHeaderField: "Content-Type")
        let (data,response) = try await session.data(for: r); _ = try check(response); return data
    }
    func pair() async throws -> String {
        let data = try await json("pair", object: ["token":invitation.token,"name":"iPhone"])
        struct Result: Decodable { let session: String; let name: String }
        let result = try JSONDecoder().decode(Result.self, from: data); token = result.session; return result.name
    }
    func next() async throws -> Envelope? {
        let (data,r) = try await session.data(for: request("next")); let status = try check(r)
        return status == 204 ? nil : try JSONDecoder().decode(Envelope.self, from: data)
    }
    func ack(_ id: String) async throws { let (_,r) = try await session.data(for: request("ack/" + id, method: "POST")); _ = try check(r) }
    func upload(_ file: URL) async throws {
        let (size, hash) = try await digestAsync(file)
        try Task.checkCancellation()
        let q = [URLQueryItem(name:"name",value:file.lastPathComponent), URLQueryItem(name:"size",value:String(size)), URLQueryItem(name:"sha256",value:hash)]
        var r = try request("upload", method:"POST", query:q); r.setValue("application/octet-stream",forHTTPHeaderField:"Content-Type")
        let (_,response) = try await session.upload(for:r, fromFile:file); _ = try check(response)
    }
    func download(_ e: Envelope, into dir: URL) async throws -> URL {
        guard UUID(uuidString:e.id) != nil, e.sha256.count == 64 else { throw TransferError.message("Некорректное описание файла") }
        let (tmp,r) = try await session.download(for: request("file/" + e.id), delegate: self)
        defer { try? FileManager.default.removeItem(at:tmp) }
        _ = try check(r)
        let (size, hash) = try await digestAsync(tmp)
        guard size == e.size, hash == e.sha256 else { throw TransferError.message("Размер или SHA-256 не совпал. Файл удалён.") }
        try Task.checkCancellation()
        let name = safeName(e.name)
        for i in 0..<10000 {
            let dest = dir.appendingPathComponent(i == 0 ? name : "\(i)_\(name)")
            if FileManager.default.fileExists(atPath: dest.path) { continue }
            try FileManager.default.moveItem(at: tmp, to:dest); return dest
        }
        throw TransferError.message("Слишком много совпадений имён")
    }
}
func safeName(_ name: String) -> String {
    let filtered = name.unicodeScalars.prefix(160).map { CharacterSet.controlCharacters.contains($0) || "/\\:".unicodeScalars.contains($0) ? "_" : String($0) }.joined().trimmingCharacters(in: CharacterSet(charactersIn:" ."))
    return filtered.isEmpty ? "file" : filtered
}
func digest(_ url: URL) throws -> (UInt64,String) {
    let file = try FileHandle(forReadingFrom:url); defer { try? file.close() }
    var h = SHA256(); var size: UInt64 = 0
    while let b = try file.read(upToCount:65536), !b.isEmpty { if Task.isCancelled { throw CancellationError() }; h.update(data:b); size += UInt64(b.count) }
    return (size,h.finalize().map { String(format:"%02x",$0) }.joined())
}
func digestAsync(_ url: URL) async throws -> (UInt64,String) {
    let worker = Task.detached(priority:.utility) { try digest(url) }
    return try await withTaskCancellationHandler { try await worker.value } onCancel: { worker.cancel() }
}
