import Foundation
import Photos

// PHAssetResource supplies the original bytes, including HEIC/HEVC/RAW.
// Live Photo's still and paired video are exported as separate original files.
private final class ResourceExport: @unchecked Sendable {
    private let lock = NSLock()
    private var id: PHAssetResourceDataRequestID?
    private var cancelled = false
    private var writeError: Error?
    private let handle: FileHandle
    init(_ url: URL) throws {
        guard FileManager.default.createFile(atPath:url.path,contents:nil) else { throw TransferError.message("Не удалось создать временный файл. Проверьте свободное место.") }
        handle = try FileHandle(forWritingTo:url)
    }
    func cancel() { lock.lock(); cancelled = true; let request = id; lock.unlock(); if let request { PHAssetResourceManager.default().cancelDataRequest(request) } }
    func run(_ resource: PHAssetResource, cloud: Bool) async throws {
        try await withTaskCancellationHandler {
            try await withCheckedThrowingContinuation { (continuation: CheckedContinuation<Void,Error>) in
                let options = PHAssetResourceRequestOptions(); options.isNetworkAccessAllowed = cloud
                let request = PHAssetResourceManager.default().requestData(for:resource,options:options,dataReceivedHandler: { data in
                    self.lock.lock(); defer { self.lock.unlock() }
                    guard !self.cancelled, self.writeError == nil else { return }
                    do { try self.handle.write(contentsOf:data) } catch { self.writeError = error }
                }, completionHandler: { error in
                    self.lock.lock(); let cancelled = self.cancelled; let writeError = self.writeError; self.lock.unlock()
                    do { try self.handle.synchronize(); try self.handle.close() } catch { continuation.resume(throwing:error); return }
                    if cancelled { continuation.resume(throwing:CancellationError()) }
                    else if let error = writeError ?? error { continuation.resume(throwing:TransferError.message("Не удалось получить оригинал: \(error.localizedDescription). Если оригинал в iCloud, включите разрешение на загрузку и проверьте интернет.")) }
                    else { continuation.resume() }
                })
                lock.lock(); id = request; let shouldCancel = cancelled; lock.unlock()
                if shouldCancel { PHAssetResourceManager.default().cancelDataRequest(request) }
            }
        } onCancel: { self.cancel() }
    }
}
func exportOriginal(_ resource: PHAssetResource, to url: URL, cloud: Bool) async throws { let export = try ResourceExport(url); try await export.run(resource,cloud:cloud) }
