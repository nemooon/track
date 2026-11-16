// Usage: swift winshot.swift <process-name> -> prints CGWindowID of its on-screen main window
import CoreGraphics
import Foundation
let name = CommandLine.arguments[1]
let list = CGWindowListCopyWindowInfo([.optionAll], kCGNullWindowID) as! [[String: Any]]
for w in list where (w[kCGWindowOwnerName as String] as? String) == name
  && (w[kCGWindowLayer as String] as? Int) == 0
  && (w[kCGWindowIsOnscreen as String] as? Bool) == true {
  print(w[kCGWindowNumber as String]!); exit(0)
}
exit(1)
