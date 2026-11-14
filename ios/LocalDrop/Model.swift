import SwiftUI
import Photos
import UniformTypeIdentifiers

@MainActor final class Model: ObservableObject {
    @Published var code = ""
    @Published var text = ""
    @Published var receivedText = ""
    @Published var status = "Не подключено" { didSet { updateLive() } }
    @Published var computer = ""
    @Published var busy = false { didSet {
        UIApplication.shared.isIdleTimerDisabled = busy && !background
        if busy {
            live.start(computer:computer,message:status)
            if budget == .invalid { budget = UIApplication.shared.beginBackgroundTask(withName:"LocalDrop transfer") { [weak self] in self?.pause("iOS завершила разрешённое время в фоне. Передача остановлена. Откройте приложение и подключитесь снова.") } }
        } else { endBudget(); live.end(message:status,done:done,total:total) }
    } }
    @Published var done: Int64 = 0 { didSet { updateLive() } }
    @Published var total: Int64 = 0 { didSet { updateLive() } }
    @Published var received: [URL] = []
    @Published var allowCloud = false
    private var transport: Transport?
    private var invitation: Invitation?
    private var sessionToken: String?
    private var listener: Task<Void,Never>?
    private var operation: Task<Void,Never>?
    private var generation = UUID()
    private var acknowledged = Set<String>()
    private let live = LiveStatus()
    private var background = false
    private var budget: UIBackgroundTaskIdentifier = .invalid
    let documents = FileManager.default.urls(for:.documentDirectory,in:.userDomainMask)[0]
    init() { refresh() }
    private func updateLive() { if busy { live.update(message:status,done:done,total:total) } }
    private func endBudget() { if budget != .invalid { UIApplication.shared.endBackgroundTask(budget); budget = .invalid } }
    func sceneChanged(_ phase: ScenePhase) {
        if phase == .background {
            background = true; UIApplication.shared.isIdleTimerDisabled = false
            if !busy { pause("В фоне ожидание остановлено. Откройте приложение и нажмите «Подключить».") }
            else if budget == .invalid { pause("iOS не разрешила продолжить передачу в фоне. Подключитесь снова в приложении.") }
        } else if phase == .active { background = false; UIApplication.shared.isIdleTimerDisabled = busy }
    }
    var connected: Bool { transport != nil && sessionToken != nil }
    func refresh() {
        received = ((try? FileManager.default.contentsOfDirectory(at:documents, includingPropertiesForKeys:nil)) ?? []).filter { !$0.lastPathComponent.hasPrefix(".") }.sorted { $0.lastPathComponent < $1.lastPathComponent }
    }
    private func configure(_ t: Transport) {
        t.progress = { [weak self] d,n in Task { @MainActor in self?.done = d; self?.total = n } }
    }
    func connect() {
        guard operation == nil, transport == nil else { return }
        let current = generation
        operation = Task {
            defer { if current == generation { operation = nil } }
            do {
                let parsed = try JSONDecoder().decode(Invitation.self, from:Data(code.utf8)); try parsed.validate()
                if let old = invitation, old.host != parsed.host || old.token != parsed.token || old.fingerprint != parsed.fingerprint { sessionToken = nil }
                invitation = parsed
                let t = Transport(parsed); configure(t); transport?.close(); transport = t
                status = sessionToken == nil ? "Подтвердите сопряжение в окне Windows (60 секунд)" : "Подключение…"
                if let token = sessionToken { t.token = token } else { computer = try await t.pair(); sessionToken = t.token }
                try Task.checkCancellation(); guard current == generation else { return }
                status = "Подключено к \(computer.isEmpty ? "Windows" : computer)"; listen(t)
            } catch { if current == generation { transport?.close(); transport = nil; status = error.localizedDescription } }
        }
    }
    private func listen(_ t: Transport) {
        listener?.cancel()
        let current = generation
        listener = Task { [weak self] in
            guard let self else { return }
            do {
                while !Task.isCancelled {
                    guard let e = try await t.next() else { continue }
                    try Task.checkCancellation()
                    if acknowledged.contains(e.id) { try await t.ack(e.id); continue }
                    done = 0; total = Int64(clamping:e.size); status = e.text == nil ? "Получение: \(e.name)" : "Получение текста"; busy = true
                    if let text = e.text { receivedText = text; status = "Получен текст" }
                    else { status = "Получение: \(e.name)"; _ = try await t.download(e, into:documents); refresh(); status = "Получено и проверено: \(e.name)" }
                    acknowledged.insert(e.id)
                    if acknowledged.count > 1024 { acknowledged = [e.id] }
                    try await t.ack(e.id)
                    busy = false
                    if background { pause("Передача завершена. Откройте приложение для следующих файлов."); return }
                }
            } catch {
                if !Task.isCancelled, transport === t { status = "Приём остановлен: \(error.localizedDescription). Нажмите «Подключить» для повтора."; transport?.close(); transport = nil }
                if current == generation { busy = false }
            }
        }
    }
    func pause(_ reason: String) {
        generation = UUID(); listener?.cancel(); listener = nil; operation?.cancel(); operation = nil
        transport?.close(); transport = nil; status = reason; busy = false
    }
    func forget() { pause("Отключено. Перед новым сопряжением нажмите «Отключить устройство» в Windows."); sessionToken = nil; invitation = nil; computer = "" }
    func cancel() { pause("Передача отменена. Незавершённые файлы не сохранены. Для продолжения нажмите «Подключить».") }
    func sendText() {
        let value = text
        guard value.utf8.count <= 1024*1024 else { status = "Текст превышает 1 МиБ"; return }
        perform { t in _ = try await t.json("text", object:["text":value]) }
    }
    func sendFiles(_ urls: [URL]) {
        guard !urls.isEmpty else { return }
        perform { t in
            for url in urls {
                try Task.checkCancellation()
                let access = url.startAccessingSecurityScopedResource(); defer { if access { url.stopAccessingSecurityScopedResource() } }
                // Coordinate reads with iCloud Drive / third-party document providers.
                let staged = try await stageDocument(url)
                defer { try? FileManager.default.removeItem(at:staged.deletingLastPathComponent()) }
                await MainActor.run { self.status = "Отправка: \(url.lastPathComponent)"; self.done = 0; self.total = 0 }
                try await t.upload(staged)
            }
        }
    }
    func sendAssets(_ identifiers: [String]) {
        guard !identifiers.isEmpty else { return }
        let cloud = allowCloud
        perform { t in
            let permission = await PHPhotoLibrary.requestAuthorization(for:.readWrite)
            guard permission == .authorized || permission == .limited else { throw TransferError.message("Нет доступа к оригиналам медиатеки. Разрешите доступ в Настройках.") }
            for id in identifiers {
                try Task.checkCancellation()
                guard let asset = PHAsset.fetchAssets(withLocalIdentifiers:[id], options:nil).firstObject else { throw TransferError.message("Оригинал недоступен. При ограниченном доступе добавьте это фото в разрешённые.") }
                let resources = PHAssetResource.assetResources(for:asset).filter { [.photo,.video,.pairedVideo,.alternatePhoto,.audio].contains($0.type) }
                guard !resources.isEmpty else { throw TransferError.message("Оригинальные ресурсы не найдены") }
                for resource in resources {
                    let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString, isDirectory:true)
                    try FileManager.default.createDirectory(at:dir,withIntermediateDirectories:true)
                    defer { try? FileManager.default.removeItem(at:dir) }
                    let file = dir.appendingPathComponent(safeName(resource.originalFilename))
                    await MainActor.run { self.status = "Подготовка оригинала: \(resource.originalFilename)"; self.done = 0; self.total = 0 }
                    try await exportOriginal(resource, to:file, cloud:cloud)
                    try Task.checkCancellation()
                    await MainActor.run { self.status = "Отправка: \(resource.originalFilename)" }
                    try await t.upload(file)
                }
            }
        }
    }
    private func perform(_ work: @escaping (Transport) async throws -> Void) {
        guard operation == nil, let t = transport, sessionToken != nil else { status = "Сначала подключите компьютер"; return }
        let receiver = listener; receiver?.cancel(); listener = nil
        let current = generation
        operation = Task {
            // Wait for any receive to relinquish the server's single transfer slot.
            await receiver?.value
            guard current == generation else { return }
            done = 0; total = 0; busy = true
            do { try await work(t); try Task.checkCancellation(); status = "Отправлено" }
            catch { if current == generation { status = "Отправка остановлена: \(error.localizedDescription)" } }
            if current == generation { busy = false; operation = nil; if background { pause("Отправка завершена. Откройте приложение для продолжения.") } else { listen(t) } }
        }
    }
    func saveToPhotos(_ url: URL) {
        Task {
            do {
                let permission = await PHPhotoLibrary.requestAuthorization(for:.addOnly)
                guard permission == .authorized || permission == .limited else { throw TransferError.message("Нет разрешения на сохранение в медиатеку") }
                let type = UTType(filenameExtension:url.pathExtension)
                guard type?.conforms(to:.image) == true || type?.conforms(to:.movie) == true else { throw TransferError.message("Этот формат сохраняется через приложение «Файлы»") }
                try await PHPhotoLibrary.shared().performChanges {
                    let r = PHAssetCreationRequest.forAsset()
                    r.addResource(with: type?.conforms(to:.image) == true ? .photo : .video, fileURL:url, options:nil)
                }
                status = "Сохранено в медиатеку"
            } catch { status = "Не удалось сохранить: \(error.localizedDescription)" }
        }
    }
}

func stageDocument(_ url: URL) async throws -> URL {
    let worker = Task.detached(priority:.utility) {
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString,isDirectory:true)
        try FileManager.default.createDirectory(at:dir,withIntermediateDirectories:true)
        let dest = dir.appendingPathComponent(safeName(url.lastPathComponent))
        var coordinationError: NSError?; var copyError: Error?
        NSFileCoordinator().coordinate(readingItemAt:url,options:[],error:&coordinationError) { source in
            do {
                try Task.checkCancellation()
                let input = try FileHandle(forReadingFrom:source); defer { try? input.close() }
                guard FileManager.default.createFile(atPath:dest.path,contents:nil) else { throw TransferError.message("Не удалось создать временную копию") }
                let output = try FileHandle(forWritingTo:dest); defer { try? output.close() }
                while let block = try input.read(upToCount:65536), !block.isEmpty { try Task.checkCancellation(); try output.write(contentsOf:block) }
                try output.synchronize()
            } catch { copyError = error }
        }
        if let error = coordinationError ?? (copyError as NSError?) { try? FileManager.default.removeItem(at:dir); throw error }
        return dest
    }
    return try await withTaskCancellationHandler { try await worker.value } onCancel: { worker.cancel() }
}
