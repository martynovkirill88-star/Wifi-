import SwiftUI
import Photos

@main struct LocalDropApp: App {
    @StateObject private var model = Model()
    @Environment(\.scenePhase) private var phase
    var body: some Scene {
        WindowGroup {
            ContentView(model:model)
                .onChange(of:phase) { p in model.sceneChanged(p) }
        }
    }
}
struct ContentView: View {
    @ObservedObject var model: Model
    @State private var scanner = false
    @State private var photos = false
    @State private var files = false
    var body: some View {
        NavigationStack {
            Form {
                Section("Компьютер") {
                    Text(model.status).textSelection(.enabled)
                    if !model.computer.isEmpty { Label(model.computer,systemImage:"desktopcomputer") }
                    HStack {
                        Button("Сканировать QR") { scanner = true }
                        Button("Подключить") { model.connect() }
                    }.buttonStyle(.bordered)
                    DisclosureGroup("Ручное подключение") {
                        Text("Скопируйте весь код из окна Windows. IP, порт, отпечаток и секрет нужны вместе.").font(.caption)
                        TextEditor(text:$model.code).frame(minHeight:90).font(.caption.monospaced()).autocorrectionDisabled()
                        PasteButton(payloadType:String.self) { values in model.code = values.first ?? "" }
                    }
                    Button("Отключить и забыть сопряжение",role:.destructive) { model.forget() }
                }
                Section("Отправить") {
                    Button("Фото и видео — оригиналы") {
                        Task { _ = await PHPhotoLibrary.requestAuthorization(for:.readWrite); photos = true }
                    }.disabled(!model.connected || model.busy)
                    Toggle("Разрешить загрузку оригиналов из iCloud",isOn:$model.allowCloud)
                    Text("Оригиналы из iCloud требуют интернета и места на iPhone. Live Photo отправляется фото и отдельным видео. Изменённые версии не экспортируются.").font(.caption).foregroundStyle(.secondary)
                    Button("Выбрать файлы") { files = true }.disabled(!model.connected || model.busy)
                    TextEditor(text:$model.text).frame(minHeight:85)
                    HStack {
                        PasteButton(payloadType:String.self) { model.text = $0.first ?? "" }
                        Button("Отправить текст") { model.sendText() }.disabled(!model.connected || model.busy)
                    }
                }
                if model.busy {
                    Section("Передача") {
                        Text("Можно заблокировать экран: iOS может дать ограниченное время для завершения. Live Activity показывает прогресс, но не продлевает это время.").font(.caption)
                        if model.total > 0 { ProgressView(value:Double(model.done),total:Double(max(model.total,1))) }
                        else { ProgressView() }
                        Text("\(ByteCountFormatter.string(fromByteCount:model.done,countStyle:.file)) / \(ByteCountFormatter.string(fromByteCount:model.total,countStyle:.file))")
                        Button("Отменить",role:.destructive) { model.cancel() }
                    }
                }
                if !model.receivedText.isEmpty {
                    Section("Полученный текст") {
                        Text(model.receivedText).textSelection(.enabled)
                        Button("Копировать") { UIPasteboard.general.string = model.receivedText }
                    }
                }
                Section("Полученные файлы") {
                    Text("Файлы доступны в «Файлы → На моём iPhone → LocalDrop». Через «Поделиться» можно выбрать «Сохранить в Файлы».").font(.caption)
                    ForEach(model.received,id:\.self) { url in
                        VStack(alignment:.leading) {
                            Text(url.lastPathComponent).lineLimit(2)
                            HStack {
                                ShareLink("Поделиться / сохранить",item:url)
                                Button("В медиатеку") { model.saveToPhotos(url) }
                            }.font(.caption).buttonStyle(.bordered)
                        }
                    }
                }
            }.navigationTitle("LocalDrop")
            .sheet(isPresented:$scanner) {
                NavigationStack {
                    QRScanner { code in model.code = code; scanner = false; model.connect() }
                        .navigationTitle("QR на компьютере")
                        .toolbar { Button("Закрыть") { scanner = false } }
                }
            }
            .sheet(isPresented:$photos) { OriginalPicker { ids in photos = false; model.sendAssets(ids) } }
            .fileImporter(isPresented:$files,allowedContentTypes:[.item],allowsMultipleSelection:true) { result in
                switch result { case .success(let urls): model.sendFiles(urls); case .failure(let error): model.status = error.localizedDescription }
            }
        }
    }
}
