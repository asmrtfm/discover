// Import forms
import Foundation
import UIKit
import SwiftUI

// Protocol
protocol Identifiable {
    var id: String { get }
    func describe() -> String
}

// Struct
struct User: Identifiable, Codable {
    let id: String
    let name: String
    var email: String?

    func describe() -> String {
        return "User(\(name))"
    }

    static func guest() -> User {
        return User(id: "0", name: "Guest")
    }
}

// Enum
enum NetworkError: Error {
    case timeout
    case notFound(url: String)
    case unauthorized
    case serverError(code: Int, message: String)
}

// Class
class NetworkClient {
    let baseURL: URL
    var session: URLSession

    init(baseURL: URL) {
        self.baseURL = baseURL
        self.session = .shared
    }

    func fetch(path: String) async throws -> Data {
        let url = baseURL.appendingPathComponent(path)
        let (data, _) = try await session.data(from: url)
        return data
    }

    class func shared() -> NetworkClient {
        return NetworkClient(baseURL: URL(string: "https://api.example.com")!)
    }
}

// Actor
actor CacheManager {
    private var store: [String: Data] = [:]

    func get(key: String) -> Data? {
        return store[key]
    }

    func set(key: String, value: Data) {
        store[key] = value
    }
}

// Extension
extension String {
    var isBlank: Bool {
        return trimmed.isEmpty
    }

    var trimmed: String {
        return trimmingCharacters(in: .whitespacesAndNewlines)
    }
}

extension User {
    var displayName: String {
        return email.map { "\(name) <\($0)>" } ?? name
    }
}

// Top-level functions
func createApp() -> some View {
    return Text("Hello")
}

func configure(client: NetworkClient, cache: CacheManager) async {
    await cache.set(key: "config", value: Data())
}
