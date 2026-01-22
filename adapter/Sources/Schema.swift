import Foundation

struct AgentRequest: Decodable {
    let id: String
    let action: String
    let payload: [String: AnyCodable]?
}

struct AgentResponse: Encodable {
    let request_id: String
    let status: String
    let data: AnyCodable?
    let error: String?
}

// Minimal implementation of AnyCodable for now
struct AnyCodable: Codable {
    let value: Any

    init(_ value: Any) {
        self.value = value
    }

    func encode(to encoder: Encoder) throws {
        // Implement encryption logic if needed for specific types, for now just a stub
        var container = encoder.singleValueContainer()
        if let intVal = value as? Int {
            try container.encode(intVal)
        } else if let stringVal = value as? String {
            try container.encode(stringVal)
        } else if let boolVal = value as? Bool {
            try container.encode(boolVal)
        } else {
             // Fallback or error
        }
    }

    init(from decoder: Decoder) throws {
        let container = try decoder.singleValueContainer()
        if let intVal = try? container.decode(Int.self) {
            value = intVal
        } else if let stringVal = try? container.decode(String.self) {
            value = stringVal
        } else if let boolVal = try? container.decode(Bool.self) {
            value = boolVal
        } else {
            value = "" // default
        }
    }
}
