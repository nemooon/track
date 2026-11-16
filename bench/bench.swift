// bench key <pid> <windowID> <n>      : ⌘] then ⌘[ ; ms from key post to the next changed frame
// bench hover|scroll <pid> <windowID> <secs> : synthetic pointer load inside the window; prints frames delivered
import AppKit
import ScreenCaptureKit
import CoreMedia

final class Watcher: NSObject, SCStreamOutput {
    var lastFrame = Date.distantPast
    var frames = 0
    var lastHash: UInt64 = 0
    var waiting: Date? = nil
    var latency: Double? = nil
    var times: [Date] = []
    func stream(_ s: SCStream, didOutputSampleBuffer sb: CMSampleBuffer, of type: SCStreamOutputType) {
        guard type == .screen,
              let att = CMSampleBufferGetSampleAttachmentsArray(sb, createIfNecessary: false) as? [[SCStreamFrameInfo: Any]],
              let raw = att.first?[.status] as? Int, let st = SCFrameStatus(rawValue: raw), st == .complete else { return }
        let now = Date()
        // count only frames whose pixels changed (sparse sample hash of the pixel buffer)
        guard let px = CMSampleBufferGetImageBuffer(sb) else { return }
        CVPixelBufferLockBaseAddress(px, .readOnly)
        defer { CVPixelBufferUnlockBaseAddress(px, .readOnly) }
        guard let base = CVPixelBufferGetBaseAddress(px) else { return }
        let len = CVPixelBufferGetDataSize(px)
        let p = base.assumingMemoryBound(to: UInt64.self)
        var h: UInt64 = 1469598103934665603
        var i = 0
        while i < len / 8 { h = (h ^ p[i]) &* 1099511628211; i += 61 }
        if h == lastHash { return }
        lastHash = h
        frames += 1
        times.append(now)
        if let w = waiting, latency == nil { latency = now.timeIntervalSince(w) * 1000 }
    }
}

func post(_ e: CGEvent?, _ pid: pid_t) { e?.postToPid(pid) }
func key(_ code: CGKeyCode, cmd: Bool, pid: pid_t) {
    let src = CGEventSource(stateID: .hidSystemState)
    let d = CGEvent(keyboardEventSource: src, virtualKey: code, keyDown: true); if cmd { d?.flags = .maskCommand }
    let u = CGEvent(keyboardEventSource: src, virtualKey: code, keyDown: false); if cmd { u?.flags = .maskCommand }
    post(d, pid); post(u, pid)
}

let args = CommandLine.arguments
let mode = args[1], pid = pid_t(Int32(args[2])!), wid = CGWindowID(UInt32(args[3])!), n = Double(args[4])!

_ = NSApplication.shared
let sem = DispatchSemaphore(value: 0)
Task {
    let content = try await SCShareableContent.excludingDesktopWindows(false, onScreenWindowsOnly: true)
    guard let win = content.windows.first(where: { $0.windowID == wid }) else { print("window not found"); exit(1) }
    NSRunningApplication(processIdentifier: pid)?.activate()
    try await Task.sleep(nanoseconds: 500_000_000)
    let cfg = SCStreamConfiguration()
    cfg.width = Int(win.frame.width); cfg.height = Int(win.frame.height)
    cfg.minimumFrameInterval = CMTime(value: 1, timescale: 120)
    cfg.queueDepth = 8
    cfg.showsCursor = false
    let w = Watcher()
    let stream = SCStream(filter: SCContentFilter(desktopIndependentWindow: win), configuration: cfg, delegate: nil)
    try stream.addStreamOutput(w, type: .screen, sampleHandlerQueue: DispatchQueue(label: "f"))
    try await stream.startCapture()
    try await Task.sleep(nanoseconds: 700_000_000)
    if mode == "key" {
        var first: [Double] = [], settled: [Double] = []
        for i in 0..<(Int(n) * 2) {
            try await Task.sleep(nanoseconds: 300_000_000)
            w.times.removeAll()
            let t0 = Date()
            key(i % 2 == 0 ? 30 : 33, cmd: true, pid: pid)   // 30 = ']' , 33 = '['
            try await Task.sleep(nanoseconds: 1_500_000_000)
            // frames after the key; "settled" = last frame before a 300 ms quiet gap
            let ts = w.times.filter { $0 > t0 }.map { $0.timeIntervalSince(t0) * 1000 }
            guard let f = ts.first else { continue }
            var last = f
            for t in ts.dropFirst() { if t - last > 300 { break }; last = t }
            first.append(f); settled.append(last)
        }
        func med(_ a: [Double]) -> String { let s = a.sorted(); return s.isEmpty ? "-" : String(format: "%.0f", s[s.count / 2]) }
        print("first-change ms median:", med(first), " settled ms median:", med(settled), " samples:", first.count)
        print("settled all:", settled.sorted().map { String(format: "%.0f", $0) }.joined(separator: " "))
    } else if mode == "idle" {
        let f0 = w.frames
        try await Task.sleep(nanoseconds: UInt64(n * 1_000_000_000))
        print("idle frames:", w.frames - f0, "in", n, "s")
    } else {
        let f = win.frame  // screen coords (top-left origin)
        let x0 = f.minX + 120, x1 = f.maxX - 40, y0 = f.minY + 280, y1 = f.maxY - 40
        let start = Date(); var t = 0.0; let f0 = w.frames
        while Date().timeIntervalSince(start) < n {
            t += 1
            if mode == "hover" {
                let x = x0 + (x1 - x0) * (0.5 + 0.5 * sin(t / 37)), y = y0 + (y1 - y0) * (0.5 + 0.5 * sin(t / 23))
                CGEvent(mouseEventSource: nil, mouseType: .mouseMoved, mouseCursorPosition: CGPoint(x: x, y: y), mouseButton: .left)?.post(tap: .cghidEventTap)
            } else {
                CGEvent(mouseEventSource: nil, mouseType: .mouseMoved, mouseCursorPosition: CGPoint(x: (x0 + x1) / 2, y: (y0 + y1) / 2), mouseButton: .left)?.post(tap: .cghidEventTap)
                let dy: Int32 = Int(t / 60) % 2 == 0 ? -6 : 6
                CGEvent(scrollWheelEvent2Source: nil, units: .pixel, wheelCount: 1, wheel1: dy, wheel2: 0, wheel3: 0)?.post(tap: .cghidEventTap)
            }
            try await Task.sleep(nanoseconds: 8_333_000)   // ~120 Hz input
        }
        print("\(mode) frames:", w.frames - f0, "in", n, "s =", String(format: "%.1f fps", Double(w.frames - f0) / n))
    }
    try await stream.stopCapture()
    sem.signal()
}
sem.wait()
