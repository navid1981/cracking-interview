import Foundation
import Cocoa
import ScreenCaptureKit
import UniformTypeIdentifiers

struct Options {
    var displayID: UInt32?
    var displayIndex: Int = 0
    var outPath: String = ""
    var excludePIDs: [pid_t] = []
    var unhideHidden: Bool = true
    var isThumbnail: Bool = false
    var maxDim: Int = 2000
    var quality: Float = 0.82
}

@main
struct ScreenCapturerApp {
    static func parseArgs() -> Options {
        var opts = Options()
        var args = Array(CommandLine.arguments.dropFirst())
        while !args.isEmpty {
            let arg = args.removeFirst()
            switch arg {
            case "--display-id":
                if let val = args.first, let id = UInt32(val) {
                    opts.displayID = id
                    args.removeFirst()
                }
            case "--display-index":
                if let val = args.first, let idx = Int(val) {
                    opts.displayIndex = idx
                    args.removeFirst()
                }
            case "--out":
                if let val = args.first {
                    opts.outPath = val
                    args.removeFirst()
                }
            case "--exclude-pid":
                if let val = args.first, let pid = pid_t(val) {
                    opts.excludePIDs.append(pid)
                    args.removeFirst()
                }
            case "--unhide-hidden":
                opts.unhideHidden = true
            case "--no-unhide":
                opts.unhideHidden = false
            case "--thumbnail":
                opts.isThumbnail = true
            case "--max-dim":
                if let val = args.first, let dim = Int(val) {
                    opts.maxDim = dim
                    args.removeFirst()
                }
            case "--quality":
                if let val = args.first, let q = Float(val) {
                    opts.quality = q
                    args.removeFirst()
                }
            default:
                break
            }
        }
        return opts
    }

    static func main() async {
        _ = NSApplication.shared

        let opts = parseArgs()
        guard !opts.outPath.isEmpty else {
            fputs("Error: --out path required\n", stderr)
            exit(1)
        }

        // 1. If unhideHidden is enabled, reveal hidden regular applications
        var previouslyHiddenApps: [NSRunningApplication] = []
        if opts.unhideHidden {
            previouslyHiddenApps = NSWorkspace.shared.runningApplications.filter {
                $0.activationPolicy == .regular && $0.isHidden
            }
            for app in previouslyHiddenApps {
                app.unhide()
            }
            if !previouslyHiddenApps.isEmpty {
                // Give macOS WindowServer 50ms to map windows onto framebuffer
                try? await Task.sleep(nanoseconds: 50_000_000)
            }
        }

        // Always re-hide the unhidden apps when capture is finished or on error
        defer {
            if !previouslyHiddenApps.isEmpty {
                for app in previouslyHiddenApps {
                    app.hide()
                }
            }
        }

        do {
            // 2. Query ScreenCaptureKit content
            let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: false)
            guard !content.displays.isEmpty else {
                fputs("Error: No displays found in ScreenCaptureKit\n", stderr)
                exit(2)
            }

            // 3. Locate target display
            let targetDisplay: SCDisplay
            if let targetID = opts.displayID, let match = content.displays.first(where: { $0.displayID == targetID }) {
                targetDisplay = match
            } else if opts.displayIndex >= 0 && opts.displayIndex < content.displays.count {
                targetDisplay = content.displays[opts.displayIndex]
            } else {
                targetDisplay = content.displays[0]
            }

            // 4. Exclude our app's windows (and any other specified PIDs)
            let appsToExclude = content.applications.filter { app in
                opts.excludePIDs.contains(app.processID)
            }

            let filter: SCContentFilter
            if !appsToExclude.isEmpty {
                filter = SCContentFilter(display: targetDisplay, excludingApplications: appsToExclude, exceptingWindows: [])
            } else {
                filter = SCContentFilter(display: targetDisplay, excludingWindows: [])
            }

            // 5. Configure stream capture parameters
            let conf = SCStreamConfiguration()
            conf.showsCursor = false

            if opts.isThumbnail {
                conf.width = 160
                conf.height = 120
            } else {
                // Target retina resolution (2x native points) capped by maxDim
                var width = targetDisplay.width * 2
                var height = targetDisplay.height * 2
                let maxD = opts.maxDim
                if width > maxD || height > maxD {
                    let scale = Double(maxD) / Double(max(width, height))
                    width = Int(Double(width) * scale)
                    height = Int(Double(height) * scale)
                }
                conf.width = width
                conf.height = height
            }

            // 6. Capture the screen image
            let cgImage = try await SCScreenshotManager.captureImage(contentFilter: filter, configuration: conf)

            // 7. Compress to JPEG
            let bitmapRep = NSBitmapImageRep(cgImage: cgImage)
            guard let jpegData = bitmapRep.representation(using: .jpeg, properties: [.compressionFactor: NSNumber(value: opts.quality)]) else {
                fputs("Error: Failed to compress CGImage to JPEG\n", stderr)
                exit(3)
            }

            // 8. Write to output file
            let outURL = URL(fileURLWithPath: opts.outPath)
            try jpegData.write(to: outURL)

            // Allow WindowServer IPC queue to flush the hide command if needed
            if !previouslyHiddenApps.isEmpty {
                for app in previouslyHiddenApps {
                    app.hide()
                }
                try? await Task.sleep(nanoseconds: 50_000_000)
            }
            exit(0)
        } catch {
            fputs("Capture error: \(error.localizedDescription)\n", stderr)
            if !previouslyHiddenApps.isEmpty {
                for app in previouslyHiddenApps {
                    app.hide()
                }
                try? await Task.sleep(nanoseconds: 50_000_000)
            }
            exit(4)
        }
    }
}
