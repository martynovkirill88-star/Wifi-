import XCTest
import UIKit
import Combine
@testable import LocalDrop

private final class FileTransferProtocol: URLProtocol {
    static var onReceive:(()->Void)?
    static var onUpload:((URL)->Void)?
    override class func canInit(with request:URLRequest)->Bool { true }
    override class func canonicalRequest(for request:URLRequest)->URLRequest { request }
    override func startLoading() {
        guard let url = request.url else { return }
        if url.path == "/v1/next" { Self.onReceive?(); return } // An idle, unfinished receive.
        if url.path == "/v1/upload" { Self.onUpload?(url) }
        let data = Data((url.path == "/v1/hello" ? "{\"v\":1}" : "{\"saved\":\"fixture.txt\"}").utf8)
        let response = HTTPURLResponse(url:url,statusCode:200,httpVersion:"HTTP/1.1",headerFields:["Content-Type":"application/json"])!
        client?.urlProtocol(self,didReceive:response,cacheStoragePolicy:.notAllowed)
        client?.urlProtocol(self,didLoad:data)
        client?.urlProtocolDidFinishLoading(self)
    }
    override func stopLoading() {}
}

@MainActor final class FileSendingTests: XCTestCase {
    func testHomeScreenIconFilesArePresentAndDeclared() {
        let icons = Bundle.main.infoDictionary?["CFBundleIcons"] as? [String:Any]
        let primary = icons?["CFBundlePrimaryIcon"] as? [String:Any]
        let files = primary?["CFBundleIconFiles"] as? [String]
        XCTAssertEqual(files?.first,"LocalDropIcon60")
        XCTAssertNotNil(UIImage(named:"LocalDropIcon60"))
        XCTAssertNotNil(Bundle.main.url(forResource:"LocalDropIcon60@3x",withExtension:"png"))
    }
    func testConfirmedFileStartsUploadWhileReceiveIsIdle() async throws {
        let receive = expectation(description:"Long-poll is waiting")
        let upload = expectation(description:"Selected file starts uploading")
        receive.assertForOverFulfill = false
        FileTransferProtocol.onReceive = { receive.fulfill() }
        FileTransferProtocol.onUpload = { url in
            let query = URLComponents(url:url,resolvingAgainstBaseURL:false)?.queryItems ?? []
            XCTAssertEqual(query.first(where:{$0.name == "name"})?.value,"fixture.txt")
            XCTAssertEqual(query.first(where:{$0.name == "size"})?.value,"13")
            upload.fulfill()
        }
        let invitation = Invitation(v:1,host:"127.0.0.1",port:45831,fingerprint:String(repeating:"0",count:64),token:String(repeating:"1",count:64))
        let model = Model(savedPairing:SavedPairing(invitation:invitation,token:"test-session",computer:"Test Windows"),makeTransport:{ invitation in
            let c = URLSessionConfiguration.ephemeral; c.protocolClasses = [FileTransferProtocol.self]
            return Transport(invitation,configuration:c)
        },savePairing:{ _ in })
        defer { model.pause("Test completed"); FileTransferProtocol.onReceive = nil; FileTransferProtocol.onUpload = nil }
        model.connect()
        await fulfillment(of:[receive],timeout:10)
        let dir = FileManager.default.temporaryDirectory.appendingPathComponent(UUID().uuidString)
        try FileManager.default.createDirectory(at:dir,withIntermediateDirectories:true)
        defer { try? FileManager.default.removeItem(at:dir) }
        let file = dir.appendingPathComponent("fixture.txt")
        try Data("original data".utf8).write(to:file)
        model.sendFiles([file])
        XCTAssertEqual(model.selectedFileNames,["fixture.txt"])
        await fulfillment(of:[upload],timeout:15)
    }
    func testSelectionIsVisibleWhenComputerIsNotPaired() {
        let model = Model(savedPairing:nil)
        model.sendFiles([URL(fileURLWithPath:"/tmp/selected.pdf")])
        XCTAssertEqual(model.selectedFileNames,["selected.pdf"])
        XCTAssertTrue(model.status.contains("Файлы выбраны"))
        XCTAssertFalse(model.busy)
    }
}
