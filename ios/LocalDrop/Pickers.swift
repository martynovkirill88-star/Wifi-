import SwiftUI
import PhotosUI
import AVFoundation
import UniformTypeIdentifiers

struct DocumentPicker: UIViewControllerRepresentable {
    let selected: ([URL]) -> Void
    func makeCoordinator() -> Coordinator { Coordinator(selected) }
    func makeUIViewController(context:Context) -> UIDocumentPickerViewController {
        let picker = UIDocumentPickerViewController(forOpeningContentTypes:[.data],asCopy:true)
        picker.allowsMultipleSelection = true; picker.delegate = context.coordinator
        return picker
    }
    func updateUIViewController(_ controller:UIDocumentPickerViewController,context:Context) {}
    final class Coordinator: NSObject, UIDocumentPickerDelegate {
        let selected: ([URL]) -> Void
        init(_ selected:@escaping ([URL])->Void) { self.selected = selected }
        func documentPicker(_ controller:UIDocumentPickerViewController,didPickDocumentsAt urls:[URL]) { selected(urls) }
        func documentPickerWasCancelled(_ controller:UIDocumentPickerViewController) { selected([]) }
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
