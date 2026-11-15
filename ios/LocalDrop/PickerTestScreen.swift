#if DEBUG
import SwiftUI

struct PickerTestScreen:View {
    @State private var showing = false
    @State private var files:[SelectedFile] = []
    @State private var result = "No selection"
    private let directory:URL = {
        let dir = FileManager.default.urls(for:.documentDirectory,in:.userDomainMask)[0].appendingPathComponent("PickerFixture")
        try! FileManager.default.createDirectory(at:dir,withIntermediateDirectories:true)
        try! Data("picker fixture bytes".utf8).write(to:dir.appendingPathComponent("LocalDrop-picker-fixture.txt"))
        return dir
    }()
    var body:some View {
        VStack {
            Button("Choose local file") { showing = true }.accessibilityIdentifier("choose-file")
            Text(result).accessibilityIdentifier("picker-result")
        }
        .sheet(isPresented:$showing,onDismiss:{
            guard !files.isEmpty else { return }
            do {
                let data = try Data(contentsOf:files[0].url)
                result = "Imported \(files.count): \(String(data:data,encoding:.utf8) ?? "")"
            } catch { result = "Read failed: \(error.localizedDescription)" }
        }) {
            DocumentPicker(initialDirectory:directory) { files = $0; showing = false }
        }
    }
}
#endif
