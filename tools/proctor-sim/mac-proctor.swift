#!/usr/bin/env swift
// macOS Proctor & Kiosk Attack Simulator for CrackingInterview
//
// Usage:
//   swift tools/proctor-sim/mac-proctor.swift <terminate|hide|force-terminate|kiosk-window|aac>

import AppKit
import Foundation

let targetBundleId = "com.crackinginterview.app"
let targetExecutableSubstring = "cracking-interview"

func findTargetApps() -> [NSRunningApplication] {
    return NSWorkspace.shared.runningApplications.filter { app in
        if app.bundleIdentifier == targetBundleId {
            return true
        }
        if let name = app.localizedName?.lowercased(), name.contains("crackinginterview") {
            return true
        }
        if let exe = app.executableURL?.lastPathComponent.lowercased(),
           exe.contains(targetExecutableSubstring) || exe.contains("crackinginterview") {
            return true
        }
        return false
    }
}

let args = CommandLine.arguments
let subcommand = args.count > 1 ? args[1] : "help"

switch subcommand {
case "terminate":
    let apps = findTargetApps()
    print("[mac-proctor] Found \(apps.count) running CrackingInterview instance(s)")
    for app in apps {
        let res = app.terminate()
        print("[mac-proctor] Called terminate() on PID=\(app.processIdentifier) -> returned \(res)")
    }
    Thread.sleep(forTimeInterval: 2.0)
    let remaining = findTargetApps().filter { !$0.isTerminated }
    print("[mac-proctor] 2s after terminate(): \(remaining.count) instance(s) alive")

case "hide":
    let apps = findTargetApps()
    print("[mac-proctor] Found \(apps.count) running CrackingInterview instance(s)")
    for app in apps {
        let res = app.hide()
        print("[mac-proctor] Called hide() on PID=\(app.processIdentifier) -> returned \(res)")
    }
    Thread.sleep(forTimeInterval: 2.0)
    let remaining = findTargetApps()
    for app in remaining {
        print("[mac-proctor] 2s after hide(): PID=\(app.processIdentifier) isHidden=\(app.isHidden)")
    }

case "force-terminate":
    let apps = findTargetApps()
    print("[mac-proctor] Found \(apps.count) running CrackingInterview instance(s)")
    for app in apps {
        let res = app.forceTerminate()
        print("[mac-proctor] Called forceTerminate() (SIGKILL) on PID=\(app.processIdentifier) -> returned \(res)")
    }
    Thread.sleep(forTimeInterval: 3.0)
    let remaining = findTargetApps().filter { !$0.isTerminated }
    print("[mac-proctor] 3s after forceTerminate(): \(remaining.count) instance(s) alive (PIDs: \(remaining.map { $0.processIdentifier }))")

case "kiosk-window":
    print("[mac-proctor] Launching legacy kiosk presentation window at NSScreenSaverWindowLevel (1000) for 8 seconds...")
    let app = NSApplication.shared
    app.setActivationPolicy(.regular)
    if let screen = NSScreen.main {
        let win = NSWindow(
            contentRect: screen.frame,
            styleMask: [.borderless],
            backing: .buffered,
            defer: false
        )
        win.level = NSWindow.Level(rawValue: 1000)
        win.backgroundColor = NSColor.black.withAlphaComponent(0.85)
        win.makeKeyAndOrderFront(nil)
        DispatchQueue.main.asyncAfter(deadline: .now() + 8.0) {
            print("[mac-proctor] Closing kiosk window.")
            NSApp.terminate(nil)
        }
        app.run()
    }

case "aac":
    print("[mac-proctor] Simulating AEAssessmentSession pre-session sweep + 10s active lock...")
    for app in findTargetApps() {
        _ = app.terminate()
    }
    let end = Date().addingTimeInterval(10.0)
    while Date() < end {
        for app in findTargetApps() {
            print("[mac-proctor][aac] Active assessment lock killing PID=\(app.processIdentifier)")
            _ = app.forceTerminate()
        }
        Thread.sleep(forTimeInterval: 0.5)
    }
    print("[mac-proctor][aac] Assessment session ended. Waiting up to 35s to verify post-session recovery...")
    Thread.sleep(forTimeInterval: 32.0)
    let recovered = findTargetApps().filter { !$0.isTerminated }
    print("[mac-proctor][aac] Post-session check: \(recovered.count) instance(s) recovered")

default:
    print("Usage: swift tools/proctor-sim/mac-proctor.swift <terminate|hide|force-terminate|kiosk-window|aac>")
}
