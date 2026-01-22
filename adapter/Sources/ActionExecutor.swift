import ApplicationServices
import Foundation

class ActionExecutor {
    func executeClick(elementId: String) throws {
        guard let element = ElementRegistry.shared.getElement(by: elementId) else {
            throw NSError(domain: "Agent", code: 404, userInfo: [NSLocalizedDescriptionKey: "Element ID stale"])
        }
        
        // 1. Semantic Click (AXPress)
        let error = AXUIElementPerformAction(element, kAXPressAction as CFString)
        if error == .success { return }
        
        // 2. Fallback: Coordinate Click
        try fallbackClick(element)
    }
    
    private func fallbackClick(_ element: AXUIElement) throws {
        // Logic to get Position/Size and trigger CGEvent
    }
}
