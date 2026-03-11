import Foundation
import AVFoundation
import CoreMedia
import ScreenCaptureKit
import CoreGraphics

// Records SYSTEM AUDIO via ScreenCaptureKit.
//
// Modes:
//   --warm         : Initialize ScreenCaptureKit and wait for "start" command on stdin
//   --stream-pcm   : Stream raw PCM16 mono 16kHz to stdout for real-time transcription
//   (no flags)     : Start recording immediately (legacy mode)
//
// Commands (warm/stream modes, via stdin):
//   start          : Begin recording/streaming
//   stop           : Stop and exit
//
// Usage:
//   # Warm mode (pre-initialize, wait for commands)
//   cracking_interview_audio_recorder --warm --out /path/to/file.wav --timeout 180
//
//   # Stream mode (output raw PCM to stdout for Deepgram)
//   cracking_interview_audio_recorder --stream-pcm --out /tmp/unused.wav --timeout 300
//
//   # Legacy mode (start immediately)
//   cracking_interview_audio_recorder --out /path/to/file.wav --timeout 180

@available(macOS 13.0, *)
final class AudioRecorder: NSObject, SCStreamOutput, SCStreamDelegate {
  private var stream: SCStream?
  private var audioFile: AVAudioFile?
  
  private var totalFrames: Int64 = 0
  private var didLogFirstBuffer: Bool = false
  private var isRecording: Bool = false
  
  private let outURL: URL
  private var timeoutSeconds: Int
  private var timeoutWorkItem: DispatchWorkItem?
  
  private let outputSampleRate: Double = 44100
  private let outputChannels: AVAudioChannelCount = 1
  private let volumeBoost: Float = 5.0
  
  // Stream mode: output raw PCM16 mono 16kHz to stdout instead of writing WAV
  var streamPCMMode: Bool = false
  private let streamSampleRate: Double = 16000
  private let stdoutHandle = FileHandle.standardOutput

  init(outURL: URL, timeoutSeconds: Int = 180) {
    self.outURL = outURL
    self.timeoutSeconds = timeoutSeconds
  }
  
  private func ensureFileReady() {
    if audioFile != nil { return }
    
    guard let format = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: outputSampleRate, channels: outputChannels, interleaved: false) else {
      fputs("Failed to create output format\n", stderr)
      return
    }
    
    do {
      try? FileManager.default.removeItem(at: outURL)
      audioFile = try AVAudioFile(forWriting: outURL, settings: format.settings)
      fputs("Output file opened: \(outURL.path)\n", stderr)
    } catch {
      fputs("Failed to create output file: \(error)\n", stderr)
    }
  }

  /// Initialize ScreenCaptureKit (warm up) but don't start recording yet
  @MainActor
  func warmUp() async throws {
    let totalStart = Date()
    fputs("[WARM] Initializing ScreenCaptureKit...\n", stderr)
    fflush(stderr)
    
    // Check Screen Recording permission
    if !CGPreflightScreenCaptureAccess() {
      fputs("Requesting Screen Recording permission...\n", stderr)
      fflush(stderr)
      let granted = CGRequestScreenCaptureAccess()
      if !granted {
        throw NSError(domain: "AudioRecorder", code: 2, userInfo: [NSLocalizedDescriptionKey: "Screen Recording permission denied"])
      }
    }

    // Get shareable content
    let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
    
    guard !content.displays.isEmpty else {
      throw NSError(domain: "AudioRecorder", code: 1, userInfo: [NSLocalizedDescriptionKey: "No displays found"])
    }
    
    // Use main display
    let mainDisplayID = CGMainDisplayID()
    let display = content.displays.first(where: { $0.displayID == mainDisplayID }) ?? content.displays[0]

    // Create filter and config
    let filter = SCContentFilter(display: display, excludingWindows: [])

    let config = SCStreamConfiguration()
    config.width = 2
    config.height = 2
    config.minimumFrameInterval = CMTime(value: 1, timescale: 1)
    config.capturesAudio = true
    config.excludesCurrentProcessAudio = false
    config.sampleRate = 48000
    config.channelCount = 2
    config.showsCursor = false

    // Create stream
    let stream = SCStream(filter: filter, configuration: config, delegate: self)
    self.stream = stream
    try stream.addStreamOutput(self, type: .audio, sampleHandlerQueue: DispatchQueue(label: "audio.queue", qos: .userInteractive))

    // Start the stream (but don't write to file yet)
    try await stream.startCapture()
    
    fputs("[WARM] Ready in \(String(format: "%.3f", Date().timeIntervalSince(totalStart)))s. Waiting for 'start' command...\n", stderr)
    fputs("WARM_READY\n", stderr)
    fflush(stderr)
  }
  
  /// Begin recording (after warm up)
  func startRecording() {
    fputs("[WARM] Recording started\n", stderr)
    fflush(stderr)
    isRecording = true
    totalFrames = 0
    didLogFirstBuffer = false
    
    // Start timeout timer
    let timeoutWork = DispatchWorkItem { [weak self] in
      fputs("Timeout reached. Stopping recording.\n", stderr)
      self?.stopRecording()
      exit(0)
    }
    self.timeoutWorkItem = timeoutWork
    DispatchQueue.main.asyncAfter(deadline: .now() + .seconds(timeoutSeconds), execute: timeoutWork)
    
    fputs("AudioRecorder capture started.\n", stderr)
    fflush(stderr)
  }

  /// Legacy mode: Initialize and start recording immediately
  @MainActor
  func start() async throws {
    let totalStart = Date()
    fputs("[TIMING] start() begin\n", stderr)
    fflush(stderr)
    
    // Check Screen Recording permission
    let permStart = Date()
    if !CGPreflightScreenCaptureAccess() {
      fputs("Requesting Screen Recording permission...\n", stderr)
      fflush(stderr)
      let granted = CGRequestScreenCaptureAccess()
      if !granted {
        throw NSError(domain: "AudioRecorder", code: 2, userInfo: [NSLocalizedDescriptionKey: "Screen Recording permission denied"])
      }
    }
    fputs("[TIMING] Permission check: \(String(format: "%.3f", Date().timeIntervalSince(permStart)))s\n", stderr)
    fflush(stderr)

    // Get shareable content
    let contentStart = Date()
    let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
    fputs("[TIMING] Get shareable content: \(String(format: "%.3f", Date().timeIntervalSince(contentStart)))s (\(content.displays.count) displays)\n", stderr)
    fflush(stderr)
    
    guard !content.displays.isEmpty else {
      throw NSError(domain: "AudioRecorder", code: 1, userInfo: [NSLocalizedDescriptionKey: "No displays found"])
    }
    
    let mainDisplayID = CGMainDisplayID()
    let display = content.displays.first(where: { $0.displayID == mainDisplayID }) ?? content.displays[0]

    let filter = SCContentFilter(display: display, excludingWindows: [])

    let config = SCStreamConfiguration()
    config.width = 2
    config.height = 2
    config.minimumFrameInterval = CMTime(value: 1, timescale: 1)
    config.capturesAudio = true
    config.excludesCurrentProcessAudio = false
    config.sampleRate = 48000
    config.channelCount = 2
    config.showsCursor = false

    let streamStart = Date()
    let stream = SCStream(filter: filter, configuration: config, delegate: self)
    self.stream = stream
    try stream.addStreamOutput(self, type: .audio, sampleHandlerQueue: DispatchQueue(label: "audio.queue", qos: .userInteractive))
    fputs("[TIMING] Stream setup: \(String(format: "%.3f", Date().timeIntervalSince(streamStart)))s\n", stderr)
    fflush(stderr)

    let captureStart = Date()
    try await stream.startCapture()
    fputs("[TIMING] startCapture(): \(String(format: "%.3f", Date().timeIntervalSince(captureStart)))s\n", stderr)
    fflush(stderr)
    
    // Start recording immediately in legacy mode
    isRecording = true
    
    // Start timeout timer
    let timeoutWork = DispatchWorkItem { [weak self] in
      fputs("Timeout reached. Stopping recording.\n", stderr)
      Task { await self?.stop(); exit(0) }
    }
    self.timeoutWorkItem = timeoutWork
    DispatchQueue.main.asyncAfter(deadline: .now() + .seconds(timeoutSeconds), execute: timeoutWork)
    
    fputs("[TIMING] Total startup: \(String(format: "%.3f", Date().timeIntervalSince(totalStart)))s\n", stderr)
    fputs("AudioRecorder capture started.\n", stderr)
    fflush(stderr)
  }

  func stream(_ stream: SCStream, didOutputSampleBuffer sampleBuffer: CMSampleBuffer, of outputType: SCStreamOutputType) {
    guard outputType == .audio else { return }
    guard CMSampleBufferDataIsReady(sampleBuffer) else { return }
    guard isRecording else { return }
    
    guard let formatDesc = CMSampleBufferGetFormatDescription(sampleBuffer),
          let asbdPtr = CMAudioFormatDescriptionGetStreamBasicDescription(formatDesc) else { return }
    
    var asbd = asbdPtr.pointee
    guard let srcFormat = AVAudioFormat(streamDescription: &asbd) else { return }
    
    let numSamples = CMSampleBufferGetNumSamples(sampleBuffer)
    guard numSamples > 0 else { return }
    
    guard let inBuffer = AVAudioPCMBuffer(pcmFormat: srcFormat, frameCapacity: AVAudioFrameCount(numSamples)) else { return }
    inBuffer.frameLength = AVAudioFrameCount(numSamples)
    
    var blockBuffer: CMBlockBuffer?
    var neededSize: Int = 0
    CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(sampleBuffer, bufferListSizeNeededOut: &neededSize, bufferListOut: nil, bufferListSize: 0, blockBufferAllocator: nil, blockBufferMemoryAllocator: nil, flags: 0, blockBufferOut: &blockBuffer)
    
    guard neededSize > 0 else { return }
    
    let raw = UnsafeMutableRawPointer.allocate(byteCount: neededSize, alignment: MemoryLayout<AudioBufferList>.alignment)
    defer { raw.deallocate() }
    raw.initializeMemory(as: UInt8.self, repeating: 0, count: neededSize)
    let sourceABLPtr = raw.bindMemory(to: AudioBufferList.self, capacity: 1)
    
    CMSampleBufferGetAudioBufferListWithRetainedBlockBuffer(sampleBuffer, bufferListSizeNeededOut: &neededSize, bufferListOut: sourceABLPtr, bufferListSize: neededSize, blockBufferAllocator: nil, blockBufferMemoryAllocator: nil, flags: 0, blockBufferOut: &blockBuffer)
    
    let srcBuffers = UnsafeMutableAudioBufferListPointer(sourceABLPtr)
    let dstABL = inBuffer.mutableAudioBufferList
    let dstBuffers = UnsafeMutableAudioBufferListPointer(dstABL)
    
    for i in 0..<min(srcBuffers.count, dstBuffers.count) {
      guard let src = srcBuffers[i].mData else { continue }
      let bytes = min(Int(srcBuffers[i].mDataByteSize), Int(dstBuffers[i].mDataByteSize))
      if let dst = dstBuffers[i].mData, bytes > 0 {
        memcpy(dst, src, bytes)
      }
    }
    
    guard let floatData = inBuffer.floatChannelData else { return }
    
    let srcChannels = Int(srcFormat.channelCount)
    let srcFrames = Int(inBuffer.frameLength)
    let srcRate = srcFormat.sampleRate
    
    if streamPCMMode {
      // Stream mode: resample to 16kHz mono Int16 and write raw PCM to stdout
      let ratio = streamSampleRate / srcRate
      let outFrameCount = Int(Double(srcFrames) * ratio)
      guard outFrameCount > 0 else { return }
      
      var pcmBytes = Data(capacity: outFrameCount * 2)
      
      for i in 0..<outFrameCount {
        let srcIdx = Double(i) / ratio
        let idx0 = Int(srcIdx)
        let frac = Float(srcIdx - Double(idx0))
        
        var s0: Float = 0
        var s1: Float = 0
        if idx0 < srcFrames {
          for ch in 0..<srcChannels { s0 += floatData[ch][idx0] }
          s0 /= Float(srcChannels)
        }
        if idx0 + 1 < srcFrames {
          for ch in 0..<srcChannels { s1 += floatData[ch][idx0 + 1] }
          s1 /= Float(srcChannels)
        } else {
          s1 = s0
        }
        
        var sample = (s0 * (1 - frac) + s1 * frac) * volumeBoost
        if sample > 0.9 { sample = 0.9 + (sample - 0.9) * 0.2 }
        else if sample < -0.9 { sample = -0.9 + (sample + 0.9) * 0.2 }
        sample = max(-0.95, min(0.95, sample))
        
        let int16Val = Int16(sample * Float(Int16.max))
        var le = int16Val.littleEndian
        pcmBytes.append(Data(bytes: &le, count: 2))
      }
      
      stdoutHandle.write(pcmBytes)
      totalFrames += Int64(outFrameCount)
      
      if !didLogFirstBuffer {
        didLogFirstBuffer = true
        fputs("Stream: first PCM chunk \(outFrameCount) frames (from \(srcFrames) @ \(srcRate)Hz -> 16kHz)\n", stderr)
        fflush(stderr)
      }
      return
    }
    
    // File mode: resample to 44.1kHz mono Float32 and write to WAV
    ensureFileReady()
    guard let file = audioFile else { return }
    
    let ratio = outputSampleRate / srcRate
    let outputFrameCount = Int(Double(srcFrames) * ratio)
    
    guard let outFormat = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: outputSampleRate, channels: outputChannels, interleaved: false),
          let outBuffer = AVAudioPCMBuffer(pcmFormat: outFormat, frameCapacity: AVAudioFrameCount(outputFrameCount)) else {
      return
    }
    outBuffer.frameLength = AVAudioFrameCount(outputFrameCount)
    
    guard let outData = outBuffer.floatChannelData else { return }
    
    for i in 0..<outputFrameCount {
      let srcIdx = Double(i) / ratio
      let idx0 = Int(srcIdx)
      let frac = Float(srcIdx - Double(idx0))
      
      var s0: Float = 0
      var s1: Float = 0
      if idx0 < srcFrames {
        for ch in 0..<srcChannels { s0 += floatData[ch][idx0] }
        s0 /= Float(srcChannels)
      }
      if idx0 + 1 < srcFrames {
        for ch in 0..<srcChannels { s1 += floatData[ch][idx0 + 1] }
        s1 /= Float(srcChannels)
      } else {
        s1 = s0
      }
      
      var sample = (s0 * (1 - frac) + s1 * frac) * volumeBoost
      
      if sample > 0.9 { sample = 0.9 + (sample - 0.9) * 0.2 }
      else if sample < -0.9 { sample = -0.9 + (sample + 0.9) * 0.2 }
      sample = max(-0.95, min(0.95, sample))
      
      outData[0][i] = sample
    }
    
    do {
      try file.write(from: outBuffer)
      totalFrames += Int64(outputFrameCount)
      
      if !didLogFirstBuffer {
        didLogFirstBuffer = true
        fputs("First buffer written: \(outputFrameCount) frames (from \(srcFrames) @ \(srcRate)Hz)\n", stderr)
      }
    } catch {
      fputs("Write error: \(error)\n", stderr)
    }
  }
  
  func stopRecording() {
    fputs("Stopping recording...\n", stderr)
    isRecording = false
    timeoutWorkItem?.cancel()
    timeoutWorkItem = nil
    audioFile = nil
    convertToInt16()
    fputs("Recording stopped. Total frames: \(totalFrames)\n", stderr)
  }

  func stop() async {
    fputs("Stopping...\n", stderr)
    
    isRecording = false
    timeoutWorkItem?.cancel()
    timeoutWorkItem = nil
    
    if let stream = self.stream {
      try? await stream.stopCapture()
    }
    self.stream = nil
    audioFile = nil
    
    convertToInt16()
    
    fputs("Recording stopped. Total frames: \(totalFrames)\n", stderr)
  }
  
  private func convertToInt16() {
    guard totalFrames > 0 else { return }
    
    do {
      let floatFile = try AVAudioFile(forReading: outURL)
      let frames = AVAudioFrameCount(floatFile.length)
      
      guard frames > 0 else { return }
      
      guard let floatFormat = AVAudioFormat(commonFormat: .pcmFormatFloat32, sampleRate: outputSampleRate, channels: outputChannels, interleaved: false),
            let floatBuffer = AVAudioPCMBuffer(pcmFormat: floatFormat, frameCapacity: frames) else {
        return
      }
      
      try floatFile.read(into: floatBuffer)
      
      guard let floatData = floatBuffer.floatChannelData else { return }
      
      guard let int16Format = AVAudioFormat(commonFormat: .pcmFormatInt16, sampleRate: outputSampleRate, channels: outputChannels, interleaved: false),
            let int16Buffer = AVAudioPCMBuffer(pcmFormat: int16Format, frameCapacity: frames) else {
        return
      }
      int16Buffer.frameLength = frames
      
      guard let int16Data = int16Buffer.int16ChannelData else { return }
      
      for i in 0..<Int(frames) {
        int16Data[0][i] = Int16(floatData[0][i] * Float(Int16.max))
      }
      
      let tempURL = outURL.deletingLastPathComponent().appendingPathComponent("temp_int16.wav")
      try? FileManager.default.removeItem(at: tempURL)
      
      let outFile = try AVAudioFile(forWriting: tempURL, settings: int16Format.settings, commonFormat: .pcmFormatInt16, interleaved: false)
      try outFile.write(from: int16Buffer)
      
      try? FileManager.default.removeItem(at: outURL)
      try FileManager.default.moveItem(at: tempURL, to: outURL)
      
      fputs("Converted to Int16: \(frames) frames\n", stderr)
    } catch {
      fputs("Int16 conversion error: \(error)\n", stderr)
    }
  }

  func stream(_ stream: SCStream, didStopWithError error: any Error) {
    fputs("Stream stopped with error: \(error)\n", stderr)
  }
}

// MARK: - Main

@available(macOS 13.0, *)
var globalRecorder: AudioRecorder?

@available(macOS 13.0, *)
@main
struct AudioRecorderApp {
  static func main() async {
    var outPath = "/tmp/cracking_interview_audio.wav"
    var timeout = 180
    var warmMode = false
    var streamPCMMode = false
    
    let args = CommandLine.arguments
    for i in 0..<args.count {
      if args[i] == "--out" && i + 1 < args.count {
        outPath = args[i + 1]
      }
      if args[i] == "--timeout" && i + 1 < args.count {
        timeout = Int(args[i + 1]) ?? 180
      }
      if args[i] == "--warm" {
        warmMode = true
      }
      if args[i] == "--stream-pcm" {
        streamPCMMode = true
      }
    }
    
    fputs("Audio Recorder starting. Output: \(outPath), Timeout: \(timeout)s, Warm: \(warmMode), Stream: \(streamPCMMode)\n", stderr)
    fflush(stderr)
    
    let recorder = AudioRecorder(outURL: URL(fileURLWithPath: outPath), timeoutSeconds: timeout)
    recorder.streamPCMMode = streamPCMMode
    globalRecorder = recorder
    
    // Handle SIGTERM for graceful shutdown
    signal(SIGTERM) { _ in
      fputs("Received SIGTERM\n", stderr)
      fflush(stderr)
      globalRecorder?.stopRecording()
      exit(0)
    }
    
    do {
      if streamPCMMode {
        // Stream mode: initialize, start immediately, output raw PCM to stdout
        try await recorder.warmUp()
        recorder.startRecording()
        
        // Read stdin for stop command
        while let line = readLine() {
          let cmd = line.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
          fputs("Stream received command: \(cmd)\n", stderr)
          fflush(stderr)
          if cmd == "stop" || cmd == "exit" || cmd == "quit" {
            recorder.stopRecording()
            exit(0)
          }
        }
        // stdin closed — stop gracefully
        recorder.stopRecording()
        exit(0)
      } else if warmMode {
        // Warm mode: initialize and wait for commands
        try await recorder.warmUp()
        
        // Read commands from stdin
        while let line = readLine() {
          let cmd = line.trimmingCharacters(in: .whitespacesAndNewlines).lowercased()
          fputs("Received command: \(cmd)\n", stderr)
          fflush(stderr)
          
          if cmd == "start" {
            recorder.startRecording()
          } else if cmd == "stop" {
            recorder.stopRecording()
            exit(0)
          } else if cmd == "exit" || cmd == "quit" {
            exit(0)
          }
        }
      } else {
        // Legacy mode: start immediately
        try await recorder.start()
        
        // Keep running until stopped
        while true {
          try await Task.sleep(nanoseconds: 100_000_000)
        }
      }
    } catch {
      fputs("Error: \(error)\n", stderr)
      exit(1)
    }
  }
}
