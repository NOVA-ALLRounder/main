import Cocoa

class ElementRegistry {
    static let shared = ElementRegistry()
    private var cache: [String: AXUIElement] = [:]
    
    func register(_ element: AXUIElement) -> String {
        let id = UUID().uuidString
        cache[id] = element
        return id
    }
    
    func getElement(by id: String) -> AXUIElement? {
        return cache[id]
    }
    
    func clear() { cache.removeAll() }
}
