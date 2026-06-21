import CryptoKit
import Foundation

enum StableHash {
    static func hash(_ string: Substring) -> String {
        let data = Data(string.utf8)
        let digest = SHA256.hash(data: data)
        return digest.compactMap { String(format: "%02x", $0) }.prefix(8).joined()
    }
}

struct ImageBlock: Equatable {
    let mediaType: String
    let base64Data: String

    var id: String {
        StableHash.hash(base64Data.prefix(200))
    }
}
