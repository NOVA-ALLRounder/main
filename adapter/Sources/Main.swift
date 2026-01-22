import Foundation

@main
struct App {
    static func main() {
        let killSwitch = KillSwitch()
        killSwitch.startMonitoring()
        
        print("Swift Adapter Started...")
        // Start IPC Loop here
        RunLoop.main.run()
    }
}
