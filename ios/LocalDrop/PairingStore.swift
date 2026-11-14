import Foundation
import Security

struct SavedPairing: Codable { let invitation:Invitation; let token:String; let computer:String }
enum PairingStore {
    static let service = "local.localdrop.pairing"
    static func load() -> SavedPairing? {
        let q:[String:Any] = [kSecClass as String:kSecClassGenericPassword,kSecAttrService as String:service,kSecAttrAccount as String:"computer",kSecReturnData as String:true,kSecMatchLimit as String:kSecMatchLimitOne]
        var result:CFTypeRef?
        guard SecItemCopyMatching(q as CFDictionary,&result) == errSecSuccess, let data = result as? Data else { return nil }
        return try? JSONDecoder().decode(SavedPairing.self,from:data)
    }
    static func save(_ value:SavedPairing) throws {
        let data = try JSONEncoder().encode(value)
        let q:[String:Any] = [kSecClass as String:kSecClassGenericPassword,kSecAttrService as String:service,kSecAttrAccount as String:"computer"]
        let status = SecItemUpdate(q as CFDictionary,[kSecValueData as String:data] as CFDictionary)
        if status == errSecItemNotFound {
            var add = q; add[kSecValueData as String] = data; add[kSecAttrAccessible as String] = kSecAttrAccessibleAfterFirstUnlockThisDeviceOnly
            guard SecItemAdd(add as CFDictionary,nil) == errSecSuccess else { throw TransferError.message("Не удалось сохранить сопряжение в Keychain") }
        } else if status != errSecSuccess { throw TransferError.message("Не удалось обновить сопряжение в Keychain: \(status)") }
    }
    static func clear() { SecItemDelete([kSecClass as String:kSecClassGenericPassword,kSecAttrService as String:service,kSecAttrAccount as String:"computer"] as CFDictionary) }
}
