import ApplicationServices
import Cocoa

class AccessibilityService {
    static let shared = AccessibilityService()
    
    func snapshot(scope: String?) -> [String: Any] {
        // 1. Get System Wide Element
        let systemWide = AXUIElementCreateSystemWide()
        
        // 2. Identify Target (Focused App or Whole System)
        // For MVP, if scope is "window" or nil, let's get the focused application's focused window.
        // If scope is "screen", maybe whole system (Warning: Slow).
        
        var targetElement: AXUIElement = systemWide
        
        if scope == "window" || scope == nil {
             if let focusedApp = getAttribute(element: systemWide, attribute: kAXFocusedApplicationAttribute) as! AXUIElement? {
                // Try to get focused window of the app
                if let focusedWindow = getAttribute(element: focusedApp, attribute: kAXFocusedWindowAttribute) as! AXUIElement? {
                    targetElement = focusedWindow
                } else {
                    targetElement = focusedApp // Fallback to app
                }
             }
        }
        
        // 3. Crawl (Recursive)
        // Depth limit is important to avoid infinite loops or huge payloads
        return crawl(element: targetElement, depth: 0, maxDepth: 5)
    }
    
    private func crawl(element: AXUIElement, depth: Int, maxDepth: Int) -> [String: Any] {
        var node: [String: Any] = [:]
        
        // Register ID
        let id = ElementRegistry.shared.register(element)
        node["id"] = id
        
        // Basic Attributes
        let role = getAttribute(element: element, attribute: kAXRoleAttribute) as? String ?? "unknown"
        node["role"] = role
        
        if let title = getAttribute(element: element, attribute: kAXTitleAttribute) as? String {
            if !title.isEmpty { node["title"] = title }
        }
        
        if let value = getAttribute(element: element, attribute: kAXValueAttribute) {
            node["value"] = "\(value)"
        }
        
        // Frame
        /*
        if let position = getAttribute(element: element, attribute: kAXPositionAttribute),
           let size = getAttribute(element: element, attribute: kAXSizeAttribute) {
            // Need to convert AXValue to CGPoint/CGSize, skipping for brevity in initial MVP
        }
        */
        
        // Recursion
        if depth < maxDepth {
            if let children = getAttribute(element: element, attribute: kAXChildrenAttribute) as? [AXUIElement] {
                let childNodes = children.map { crawl(element: $0, depth: depth + 1, maxDepth: maxDepth) }
                if !childNodes.isEmpty {
                    node["children"] = childNodes
                }
            }
        }
        
        return node
    }
    
    private func getAttribute(element: AXUIElement, attribute: String) -> Any? {
        var value: AnyObject?
        let result = AXUIElementCopyAttributeValue(element, attribute as CFString, &value)
        if result == .success {
            return value
        }
        return nil
    }
}
