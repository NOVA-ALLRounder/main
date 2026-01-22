import ApplicationServices
import Foundation

class ActionExecutor {
    static let shared = ActionExecutor()

    func executeClick(elementId: String) throws {
        guard let element = ElementRegistry.shared.getElement(by: elementId) else {
            throw NSError(domain: "Agent", code: 404, userInfo: [NSLocalizedDescriptionKey: "Element ID \(elementId) not found or stale"])
        }
        
        // 1. Semantic Click (AXPress)
        // This attempts to trigger the element's primary action programmatically (API level)
        // It's safer and less flaky than coordinate clicks, but requires the element to support it.
        let error = AXUIElementPerformAction(element, kAXPressAction as CFString)
        
        if error == .success { 
            return 
        } else {
             // If failed, we might want to log it or throw. 
             // For strict MVP, we report failure instead of blindly falling back to coordinates yet.
             throw NSError(domain: "Agent", code: Int(error.rawValue), userInfo: [NSLocalizedDescriptionKey: "AXPressAction failed: \(error.rawValue)"])
        }
    }
}
