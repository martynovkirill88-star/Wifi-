import Foundation
import Darwin

final class PeerDiscovery: NSObject, NetServiceBrowserDelegate, NetServiceDelegate {
    private let browser = NetServiceBrowser()
    private var services:[String:NetService] = [:]
    private var active = false
    private let found:(String,String)->Void
    init(found:@escaping (String,String)->Void) { self.found = found; super.init(); browser.delegate = self }
    func start() { guard !active else { return }; active = true; browser.searchForServices(ofType:"_localdrop._tcp.",inDomain:"local.") }
    func stop() { browser.stop(); services.values.forEach{$0.stop()}; services.removeAll(); active = false }
    func netServiceBrowser(_ browser:NetServiceBrowser,didFind service:NetService,moreComing:Bool) {
        services[service.name] = service; service.delegate = self; service.resolve(withTimeout:5)
    }
    func netServiceBrowser(_ browser:NetServiceBrowser,didRemove service:NetService,moreComing:Bool) { services.removeValue(forKey:service.name)?.stop() }
    func netServiceDidResolveAddress(_ sender:NetService) {
        guard sender.port == 45831,let txt = sender.txtRecordData(),let fingerprintData = NetService.dictionary(fromTXTRecord:txt)["fingerprint"],let fingerprint = String(data:fingerprintData,encoding:.utf8) else { return }
        for data in sender.addresses ?? [] {
            var host = [CChar](repeating:0,count:Int(NI_MAXHOST))
            let status = data.withUnsafeBytes { raw -> Int32 in
                guard let ptr = raw.baseAddress else { return -1 }
                return getnameinfo(ptr.assumingMemoryBound(to:sockaddr.self),socklen_t(raw.count),&host,socklen_t(host.count),nil,0,NI_NUMERICHOST)
            }
            if status == 0 {
                let ip = String(cString:host)
                if ip.split(separator:".").count == 4 { found(ip,fingerprint); break }
            }
        }
    }
}
