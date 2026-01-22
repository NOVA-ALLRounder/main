import Foundation

@main
struct App {
    static func main() {
        let killSwitch = KillSwitch()
        killSwitch.startMonitoring()
        
        // Log to stderr so we don't pollute stdout (IPC channel)
        fputs("Swift Adapter Started...\n", stderr)
        
        // IPC Loop
        while let line = readLine() {
            handleRequest(line)
        }
    }
    
    static func handleRequest(_ jsonString: String) {
        let data = jsonString.data(using: .utf8)!
        let decoder = JSONDecoder()
        let encoder = JSONEncoder()
        
        // Default error response structure
        var response = AgentResponse(request_id: "unknown", status: "error", data: nil, error: "Invalid Request")
        
        do {
            let request = try decoder.decode(AgentRequest.self, from: data)
            response = process(request: request)
        } catch {
            response = AgentResponse(request_id: "unknown", status: "error", data: nil, error: "JSON Parse Fail: \(error)")
        }
        
        if let respData = try? encoder.encode(response),
           let respString = String(data: respData, encoding: .utf8) {
            print(respString) // Send to stdout
            fflush(stdout) // Ensure immediate flush
        }
    }
    
    static func process(request: AgentRequest) -> AgentResponse {
        var resultData: AnyCodable? = nil
        var errorMsg: String? = nil
        var status = "success"
        
        switch request.action {
        case "ui.snapshot":
            // Dispatch to AccessibilityService
            let snapshot = AccessibilityService.shared.snapshot(scope: nil)
            resultData = AnyCodable(snapshot)
            
        case "ui.click":
            if let payload = request.payload,
               let elementIdAny = payload["element_id"]?.value,
               let elementId = elementIdAny as? String {
                do {
                    try ActionExecutor.shared.executeClick(elementId: elementId)
                } catch {
                    status = "failed"
                    errorMsg = "Click failed: \(error)"
                }
            } else {
                status = "failed"
                errorMsg = "Missing element_id in payload"
            }

        default:
            status = "failed"
            errorMsg = "Unknown action: \(request.action)"
        }
        
        return AgentResponse(request_id: request.id, status: status, data: resultData, error: errorMsg)
    }
}
