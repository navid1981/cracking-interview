import Foundation
import AVFoundation
import CoreMedia
import ScreenCaptureKit
import CoreGraphics
import AppKit

// Records system audio using ScreenCaptureKit until the process is terminated.
// Usage:
//   cracking_interview_audio_recorder --out /path/to/file.wav
//
// Notes:
// - Requires macOS 13+.
// - Will prompt for Screen Recording permission (ScreenCaptureKit).

@available(macOS 13.0, *)
final class AudioRecorder: NSObject, SCStreamOutput, SCStreamDelegate {
  private var stream: SCStream?
  private var audioFile: AVAudioFile?
  private var sourceFormat: AVAudioFormat?
  private var targetFormat: AVAudioFormat?
  private var converter: AVAudioConverter?
  private var totalFramesWritten: Int64 = 0
  private var didLogInit: Bool = false
  private var didLogFirstBuffer: Bool = false
  private var didLogFirstScreenBuffer: Bool = false
  private var writeErrorCount: Int = 0
  private let outURL: URL
  private let startLock = NSLock()
  private var startDidComplete: Bool = false
  private var startCompletion: ((Result<Void, any Error>) -> Void)?

  init(outURL: URL) {
    self.outURL = outURL
  }

  private func ensureOutputFileReady() {
    if audioFile != nil && targetFormat != nil { return }
    // Pre-create the output WAV so Stop never fails with "file not created",
    // even if no audio buffers arrive (e.g., user records while system audio is silent).
    let fixedRate = 48000.0
    let fixedChannels: AVAudioChannelCount = 2
    guard let dstFormat = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: fixedRate, channels: fixedChannels, interleaved: true) else {
      fputs("Failed to create fixed target PCM format\n", stderr)
      return
    }
    self.targetFormat = dstFormat
    do {
      try? FileManager.default.removeItem(at: outURL)
      self.audioFile = try AVAudioFile(forWriting: outURL, settings: dstFormat.settings, commonFormat: .pcmFormatInt16, interleaved: true)
      fputs("Audio output file opened: \(outURL.path)\n", stderr)
    } catch {
      fputs("Failed to open WAV output: \(error)\n", stderr)
    }
  }

  @MainActor
  func start() async throws {
    fputs("AudioRecorder start() begin.\n", stderr)

    // ScreenCaptureKit requires Screen Recording permission. If it's not granted,
    // startCapture() can appear to hang. Proactively request it and fail with a clear log.
    if !CGPreflightScreenCaptureAccess() {
      fputs("Screen Recording permission not granted. Requesting access...\n", stderr)
      let granted = CGRequestScreenCaptureAccess()
      if !granted {
        fputs("Screen Recording permission denied. Enable it in System Settings → Privacy & Security → Screen Recording.\n", stderr)
        throw NSError(domain: "AudioRecorder", code: 2, userInfo: [NSLocalizedDescriptionKey: "Screen Recording permission denied"])
      }
      fputs("Screen Recording permission granted.\n", stderr)
    } else {
      fputs("Screen Recording permission already granted.\n", stderr)
    }

    // Prefer the main display; we only need a "source" to enable system audio capture.
    fputs("AudioRecorder fetching shareable content...\n", stderr)
    let content = try await SCShareableContent.current
    fputs("AudioRecorder shareable content loaded. displays=\(content.displays.count)\n", stderr)
    guard let first = content.displays.first else {
      throw NSError(domain: "AudioRecorder", code: 1, userInfo: [NSLocalizedDescriptionKey: "No display found"])
    }
    let mainID = CGMainDisplayID()
    let display = content.displays.first(where: { $0.displayID == mainID }) ?? first
    fputs("AudioRecorder using display id=\(display.displayID).\n", stderr)

    let filter = SCContentFilter(display: display, excludingWindows: [])
    let config = SCStreamConfiguration()

    // Minimize video cost; still enables system audio capture.
    // Extremely tiny sizes can be flaky on some setups; keep it small but not trivial.
    config.width = 64
    config.height = 64
    config.minimumFrameInterval = CMTime(value: 1, timescale: 1)
    config.capturesAudio = true
    // Encourage system audio delivery in a well-known format.
    // (These properties exist on macOS 13+)
    config.sampleRate = 48000
    config.channelCount = 2
    config.showsCursor = false

    let stream = SCStream(filter: filter, configuration: config, delegate: self)
    self.stream = stream

    ensureOutputFileReady()

    // Some setups appear to hang on audio-only streams; adding a minimal screen output
    // makes startCapture() more reliable while keeping video cost near-zero.
    try stream.addStreamOutput(self, type: .screen, sampleHandlerQueue: DispatchQueue(label: "screen.sample.queue"))
    try stream.addStreamOutput(self, type: .audio, sampleHandlerQueue: DispatchQueue(label: "audio.sample.queue"))
    fputs("AudioRecorder starting capture...\n", stderr)
    // Avoid hanging forever inside startCapture(). Use a GCD watchdog so we don't rely
    // on Swift concurrency scheduling (which can be impacted by system-level deadlocks).
    try await withCheckedThrowingContinuation { (cont: CheckedContinuation<Void, any Error>) in
      func completeOnce(_ result: Result<Void, any Error>) {
        startLock.lock()
        defer { startLock.unlock() }
        if startDidComplete { return }
        startDidComplete = true
        startCompletion = nil
        cont.resume(with: result)
      }

      // Allow SCStreamDelegate callbacks to fail start immediately (e.g. Code=-3805).
      startLock.lock()
      startDidComplete = false
      startCompletion = completeOnce
      startLock.unlock()

      let timeoutSeconds: Double = 12
      let timeoutWork = DispatchWorkItem {
        let err = NSError(
          domain: "AudioRecorder",
          code: 3,
          userInfo: [NSLocalizedDescriptionKey: "Timed out waiting for ScreenCaptureKit startCapture(). This is usually caused by permissions, OS bugs, or capture configuration issues."]
        )
        fputs("AudioRecorder startCapture failed/timeout: \(err)\n", stderr)
        completeOnce(.failure(err))
      }
      DispatchQueue.global().asyncAfter(deadline: .now() + timeoutSeconds, execute: timeoutWork)

      Task.detached {
        do {
          try await stream.startCapture()
          timeoutWork.cancel()
          completeOnce(.success(()))
        } catch {
          timeoutWork.cancel()
          fputs("AudioRecorder startCapture failed: \(error)\n", stderr)
          completeOnce(.failure(error))
        }
      }
    }
    fputs("AudioRecorder capture started.\n", stderr)
  }

  func stop() async {
    if let stream = self.stream {
      try? await stream.stopCapture()
    }
    self.stream = nil
    self.audioFile = nil
    self.sourceFormat = nil
    self.targetFormat = nil
    self.converter = nil
    self.totalFramesWritten = 0
    self.didLogInit = false
    self.didLogFirstBuffer = false
    self.didLogFirstScreenBuffer = false
    self.writeErrorCount = 0
  }

  func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of outputType: SCStreamOutputType) {
    if outputType == .screen {
      if !didLogFirstScreenBuffer {
        didLogFirstScreenBuffer = true
        fputs("First screen buffer received.\n", stderr)
      }
      return
    }
    guard outputType == .audio else { return }
    guard CMSampleBufferDataIsReady(sampleBuffer) else { return }

    // Lazily initialize source format + converter based on first buffer.
    ensureOutputFileReady()
    if sourceFormat == nil || targetFormat == nil || converter == nil {
      guard let formatDesc = CMSampleBufferGetFormatDescription(sampleBuffer) else { return }
      guard let asbdPtr = CMAudioFormatDescriptionGetStreamBasicDescription(formatDesc) else { return }
      var asbd = asbdPtr.pointee

      guard let srcFormat = AVAudioFormat(streamDescription: &asbd) else { return }
      self.sourceFormat = srcFormat

      guard let dstFormat = self.targetFormat else { return }
      if let conv = AVAudioConverter(from: srcFormat, to: dstFormat) {
        self.converter = conv
      } else {
        fputs("Failed to create AVAudioConverter\n", stderr)
        return
      }

      if !didLogInit {
        didLogInit = true
        fputs("AudioRecorder started. srcRate=\(srcFormat.sampleRate) srcCh=\(srcFormat.channelCount) -> dstRate=\(dstFormat.sampleRate) dstCh=\(dstFormat.channelCount) out=\(outURL.path)\n", stderr)
      }
    }

    guard let srcFormat = self.sourceFormat, let dstFormat = self.targetFormat, let conv = self.converter, let file = self.audioFile else { return }

    let numSamples = CMSampleBufferGetNumSamples(sampleBuffer)
    guard numSamples > 0 else { return }

    guard let inBuffer = AVAudioPCMBuffer(pcmFormat: srcFormat, frameCapacity: AVAudioFrameCount(numSamples)) else { return }
    inBuffer.frameLength = AVAudioFrameCount(numSamples)

    // Copy audio data into inBuffer via raw AudioBufferList -> inBuffer.audioBufferList.
    // IMPORTANT: AudioBufferList is a variable-length struct. Allocate a sufficiently large buffer.
    var blockBuffer: CMBlockBuffer?
    // First pass: ask CoreMedia how many bytes are needed for the AudioBufferList.
    var neededSize: Int = 0
    let status1 = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
      sampleBuffer,
      bufferListSizeNeededOut: &neededSize,
      bufferListOut: nil,
      bufferListSize: 0,
      blockBufferAllocator: nil,
      blockBufferMemoryAllocator: nil,
      flags: 0,
      blockBufferOut: &blockBuffer
    )
    if status1 != noErr || neededSize <= 0 {
      if writeErrorCount < 3 {
        writeErrorCount += 1
        fputs("CMSampleBufferGetAudioBufferList size query failed: status=\(status1) needed=\(neededSize)\n", stderr)
      }
      return
    }

    let raw = UnsafeMutableRawPointer.allocate(byteCount: neededSize, alignment: MemoryLayout<AudioBufferList>.alignment)
    defer { raw.deallocate() }
    raw.initializeMemory(as: UInt8.self, repeating: 0, count: neededSize)
    let sourceABLPtr = raw.bindMemory(to: AudioBufferList.self, capacity: 1)

    let status2 = CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(
      sampleBuffer,
      bufferListSizeNeededOut: &neededSize,
      bufferListOut: sourceABLPtr,
      bufferListSize: neededSize,
      blockBufferAllocator: nil,
      blockBufferMemoryAllocator: nil,
      flags: 0,
      blockBufferOut: &blockBuffer
    )
    if status2 != noErr {
      if writeErrorCount < 3 {
        writeErrorCount += 1
        fputs("CMSampleBufferGetAudioBufferList fill failed: status=\(status2) size=\(neededSize)\n", stderr)
      }
      return
    }

    let srcBuffers = UnsafeMutableAudioBufferListPointer(sourceABLPtr)
    let dstABL = inBuffer.mutableAudioBufferList
    let dstBuffers = UnsafeMutableAudioBufferListPointer(dstABL)

    // Copy buffer-by-buffer (works for both interleaved and planar formats).
    let count = min(srcBuffers.count, dstBuffers.count)
    for i in 0..<count {
      guard let src = srcBuffers[i].mData else { continue }
      let bytes = min(Int(srcBuffers[i].mDataByteSize), Int(dstBuffers[i].mDataByteSize))
      if let dst = dstBuffers[i].mData, bytes > 0 {
        memcpy(dst, src, bytes)
      }
    }

    if !didLogFirstBuffer {
      // Log first buffer info once (helps diagnose "header only" files).
      didLogFirstBuffer = true
      let srcBytes = (0..<srcBuffers.count).map { Int(srcBuffers[$0].mDataByteSize) }.reduce(0, +)
      let dstBytes = (0..<dstBuffers.count).map { Int(dstBuffers[$0].mDataByteSize) }.reduce(0, +)
      fputs("First audio buffer received: samples=\(numSamples) neededABL=\(neededSize) srcBuffers=\(srcBuffers.count) srcBytes=\(srcBytes) dstBuffers=\(dstBuffers.count) dstBytes=\(dstBytes)\n", stderr)
    }

    do {
      // Convert to Int16 PCM if needed.
      guard let outBuffer = AVAudioPCMBuffer(pcmFormat: dstFormat, frameCapacity: inBuffer.frameCapacity) else { return }
      var error: NSError?

      var didProvide = false
      let inputBlock: AVAudioConverterInputBlock = { _, outStatus in
        if didProvide {
          outStatus.pointee = .endOfStream
          return nil
        }
        didProvide = true
        outStatus.pointee = .haveData
        return inBuffer
      }

      let _ = conv.convert(to: outBuffer, error: &error, withInputFrom: inputBlock)
      if let error = error {
        if writeErrorCount < 3 {
          writeErrorCount += 1
          fputs("Audio convert error: \(error)\n", stderr)
        }
        return
      }

      try file.write(from: outBuffer)
      totalFramesWritten += Int64(outBuffer.frameLength)
    } catch {
      if writeErrorCount < 3 {
        writeErrorCount += 1
        fputs("Audio write error: \(error)\n", stderr)
      }
    }
  }

  func stream(_ stream: SCStream, didStopWithError error: any Error) {
    fputs("SCStream didStopWithError: \(error)\n", stderr)

    // If startCapture is currently pending, fail it immediately so we don't sit in a timeout loop.
    startLock.lock()
    let completion = startCompletion
    startLock.unlock()
    if let completion {
      completion(.failure(error))
    }
  }
}

func parseOutPath() -> String? {
  let args = CommandLine.arguments
  if let idx = args.firstIndex(of: "--out"), idx + 1 < args.count {
    return args[idx + 1]
  }
  return nil
}

@available(macOS 13.0, *)
@main
struct Main {
  static func main() {
    // ScreenCaptureKit tends to behave more reliably when AppKit is initialized,
    // even in a CLI helper.
    _ = NSApplication.shared
    NSApp.setActivationPolicy(.prohibited)

    guard let out = parseOutPath() else {
      fputs("Missing --out /path/to/file.wav\n", stderr)
      exit(2)
    }

    let url = URL(fileURLWithPath: out)
    let recorder = AudioRecorder(outURL: url)

    // Stop gracefully on termination signals.
    signal(SIGINT, SIG_IGN)
    signal(SIGTERM, SIG_IGN)

    let term = DispatchSource.makeSignalSource(signal: SIGTERM, queue: .main)
    term.setEventHandler {
      Task { await recorder.stop(); exit(0) }
    }
    term.resume()

    let intr = DispatchSource.makeSignalSource(signal: SIGINT, queue: .main)
    intr.setEventHandler {
      Task { await recorder.stop(); exit(0) }
    }
    intr.resume()

    Task {
      do {
        try await recorder.start()
      } catch {
        fputs("Failed to start capture: \(error)\n", stderr)
        exit(1)
      }
    }

    dispatchMain()
  }
}


