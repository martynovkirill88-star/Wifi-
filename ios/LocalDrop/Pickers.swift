import SwiftUI
import PhotosUI
import AVFoundation
import UniformTypeIdentifiers

struct DocumentPicker: UIViewControllerRepresentable {
    var initialDirectory:URL? = nil
    let selected: ([SelectedFile]) -> Void
    func makeUIViewController(context:Context) -> DocumentPickerHost { DocumentPickerHost(initialDirectory:initialDirectory,selected:selected) }
    func updateUIViewController(_ controller:DocumentPickerHost,context:Context) {}
}
final class SelectedFile {
    let url:URL
    private let scoped:Bool
    init(_ url:URL) { self.url = url; scoped = url.startAccessingSecurityScopedResource() }
    deinit { if scoped { url.stopAccessingSecurityScopedResource() } }
}
final class DocumentPickerHost: UIViewController, UIDocumentPickerDelegate {
    private let selected:([SelectedFile])->Void
    private let initialDirectory:URL?
    private var presented = false
    private var finished = false
    init(initialDirectory:URL?,selected:@escaping ([SelectedFile])->Void) { self.initialDirectory = initialDirectory; self.selected = selected; super.init(nibName:nil,bundle:nil) }
    required init?(coder:NSCoder) { fatalError("init(coder:) is unsupported") }
    override func viewDidAppear(_ animated:Bool) {
        super.viewDidAppear(animated)
        guard !presented else { return }; presented = true
        let picker = UIDocumentPickerViewController(forOpeningContentTypes:[.data],asCopy:false)
        picker.allowsMultipleSelection = true
        picker.shouldShowFileExtensions = true
        picker.directoryURL = initialDirectory
        picker.delegate = self
        picker.modalPresentationStyle = .fullScreen
        present(picker,animated:true)
    }
    func documentPicker(_ controller:UIDocumentPickerViewController,didPickDocumentsAt urls:[URL]) { complete(controller,urls:urls) }
    func documentPicker(_ controller:UIDocumentPickerViewController,didPickDocumentAt url:URL) { complete(controller,urls:[url]) }
    func documentPickerWasCancelled(_ controller:UIDocumentPickerViewController) { complete(controller,urls:[]) }
    private func complete(_ controller:UIDocumentPickerViewController,urls:[URL]) {
        guard !finished else { return }; finished = true
        // Keep access alive before dismissing the system provider. The lease is
        // retained through the SwiftUI dismissal and the entire staging task.
        let files = urls.map(SelectedFile.init)
        if controller.presentingViewController != nil {
            controller.dismiss(animated:true) { self.selected(files) }
        } else { selected(files) }
    }
}

struct OriginalPicker: UIViewControllerRepresentable {
    let selected: ([String]) -> Void
    func makeUIViewController(context:Context) -> PHPickerViewController {
        var c = PHPickerConfiguration(photoLibrary:.shared()); c.selectionLimit = 0; c.filter = .any(of:[.images,.videos]); c.preferredAssetRepresentationMode = .current
        let vc = PHPickerViewController(configuration:c); vc.delegate = context.coordinator; return vc
    }
    func updateUIViewController(_ vc:PHPickerViewController,context:Context) {}
    func makeCoordinator() -> Coordinator { Coordinator(selected) }
    final class Coordinator: NSObject, PHPickerViewControllerDelegate {
        let selected: ([String]) -> Void
        init(_ selected:@escaping ([String])->Void) { self.selected = selected }
        func picker(_ picker:PHPickerViewController,didFinishPicking results:[PHPickerResult]) {
            picker.dismiss(animated:true)
            selected(results.compactMap(\.assetIdentifier))
        }
    }
}
struct QRScanner: UIViewControllerRepresentable {
    let result: (String)->Void
    func makeUIViewController(context:Context) -> ScannerController { let c = ScannerController(); c.result = result; return c }
    func updateUIViewController(_ uiViewController:ScannerController,context:Context) {}
}
final class ScannerController: UIViewController, AVCaptureMetadataOutputObjectsDelegate {
    var result: ((String)->Void)?
    private let capture = AVCaptureSession()
    private let queue = DispatchQueue(label:"localdrop.camera")
    private var preview: AVCaptureVideoPreviewLayer?
    private var found = false
    override func viewDidLoad() {
        super.viewDidLoad(); view.backgroundColor = .black
        AVCaptureDevice.requestAccess(for:.video) { [weak self] allowed in
            guard let self else { return }
            guard allowed else { DispatchQueue.main.async { self.result?("Камера недоступна. Используйте ручной код.") }; return }
            self.queue.async {
                guard let camera = AVCaptureDevice.default(for:.video), let input = try? AVCaptureDeviceInput(device:camera), self.capture.canAddInput(input) else { return }
                self.capture.addInput(input); let output = AVCaptureMetadataOutput()
                guard self.capture.canAddOutput(output) else { return }
                self.capture.addOutput(output); output.setMetadataObjectsDelegate(self,queue:.main); output.metadataObjectTypes = [.qr]
                DispatchQueue.main.async {
                    let layer = AVCaptureVideoPreviewLayer(session:self.capture); layer.videoGravity = .resizeAspectFill; layer.frame = self.view.bounds; self.view.layer.addSublayer(layer); self.preview = layer
                }
                self.capture.startRunning()
            }
        }
    }
    override func viewDidLayoutSubviews() { super.viewDidLayoutSubviews(); preview?.frame = view.bounds }
    override func viewWillDisappear(_ animated:Bool) { super.viewWillDisappear(animated); queue.async { self.capture.stopRunning() } }
    func metadataOutput(_ output:AVCaptureMetadataOutput,didOutput metadataObjects:[AVMetadataObject],from connection:AVCaptureConnection) {
        guard !found, let code = (metadataObjects.first as? AVMetadataMachineReadableCodeObject)?.stringValue else { return }; found = true; result?(code)
    }
}
