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

/// Sends a real SIGKILL via kill(2) and reports whether it was delivered.
///
/// `NSRunningApplication.forceTerminate()` only *requests* termination via
/// LaunchServices: for processes not launched by LaunchServices (e.g. a
/// `cargo run` dev binary) it returns `true` yet delivers no SIGKILL at all
/// (the target merely gets a polite SIGTERM it can handle), which makes it
/// useless for simulating a hard kill. kill(2) cannot be caught or ignored.
@discardableResult
func sendSigkill(_ pid: pid_t) -> Bool {
    if kill(pid, SIGKILL) == 0 {
        return true
    }
    let err = errno
    print("[mac-proctor] kill(\(pid), SIGKILL) failed: errno=\(err) (\(String(cString: strerror(err))))")
    return false
}

/// Process-table truth via sysctl(KERN_PROC_PID), unlike NSWorkspace entries
/// which go stale for unbundled dev processes (dead PIDs keep reporting as
/// "alive"). Zombies count as dead — they hold no window and run no code.
func isProcessAlive(_ pid: pid_t) -> Bool {
    var info = kinfo_proc()
    var size = MemoryLayout<kinfo_proc>.size
    var mib: [Int32] = [CTL_KERN, KERN_PROC, KERN_PROC_PID, pid]
    guard sysctl(&mib, u_int(mib.count), &info, &size, nil, 0) == 0, size > 0 else {
        return false
    }
    return info.kp_proc.p_stat != 5 // 5 == SZOMB
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
    let targetPids = apps.map { $0.processIdentifier }
    for pid in targetPids {
        let ok = sendSigkill(pid)
        print("[mac-proctor] Sent SIGKILL via kill(2) to PID=\(pid) -> \(ok ? "delivered" : "FAILED")")
    }
    Thread.sleep(forTimeInterval: 3.0)
    let survivors = targetPids.filter { isProcessAlive($0) }
    print("[mac-proctor] 3s after SIGKILL: \(survivors.count) of \(targetPids.count) original PID(s) still alive (process-table check): \(survivors)")
    let respawned = findTargetApps().map { $0.processIdentifier }.filter { !targetPids.contains($0) }
    if !respawned.isEmpty {
        print("[mac-proctor] Guardian recovery detected: new instance PID(s) \(respawned)")
    }

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
            print("[mac-proctor][aac] Active assessment lock SIGKILLing PID=\(app.processIdentifier)")
            sendSigkill(app.processIdentifier)
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
