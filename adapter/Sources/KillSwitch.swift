import Cocoa

class KillSwitch {
    func startMonitoring() {
        NSEvent.addGlobalMonitorForEvents(matching: .keyDown) { event in
            // Cmd + Option + Esc (KeyCode 53)
            if event.modifierFlags.contains([.command, .option]) && event.keyCode == 53 {
                print("🚨 Kill Switch Triggered. Exiting...")
                exit(1)
            }
        }
    }
}
