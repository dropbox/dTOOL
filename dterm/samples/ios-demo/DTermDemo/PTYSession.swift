#if os(macOS)
/*
 * PTYSession.swift - Minimal PTY wrapper for macOS demo
 *
 * Copyright 2024 Andrew Yates
 * Licensed under Apache 2.0
 */

import Foundation
import Darwin

enum PTYError: Error {
    case spawnFailed(Int32)
}

final class PTYSession {
    private let master: Int32
    private let childPid: pid_t
    private let queue = DispatchQueue(label: "dterm.pty.read")
    private var readSource: DispatchSourceRead?

    var onData: ((Data) -> Void)?
    var onExit: (() -> Void)?

    init(rows: Int, cols: Int, command: [String], env: [String: String] = [:]) throws {
        var master: Int32 = 0
        var window = winsize(
            ws_row: UInt16(rows),
            ws_col: UInt16(cols),
            ws_xpixel: 0,
            ws_ypixel: 0
        )
        let pid = forkpty(&master, nil, nil, &window)
        if pid < 0 {
            throw PTYError.spawnFailed(errno)
        }

        self.master = master
        self.childPid = pid

        if pid == 0 {
            setenv("TERM", "xterm-256color", 1)
            setenv("COLORTERM", "truecolor", 1)
            for (key, value) in env {
                setenv(key, value, 1)
            }
            PTYSession.execCommand(command)
        }
        setupReadSource()
    }

    deinit {
        readSource?.cancel()
    }

    func write(_ data: Data) {
        guard !data.isEmpty else { return }
        data.withUnsafeBytes { buffer in
            guard let baseAddress = buffer.baseAddress else { return }
            var remaining = buffer.count
            var offset = 0
            while remaining > 0 {
                let written = Darwin.write(master, baseAddress.advanced(by: offset), remaining)
                if written > 0 {
                    remaining -= written
                    offset += written
                } else if errno != EINTR {
                    break
                }
            }
        }
    }

    func resize(rows: Int, cols: Int) {
        var window = winsize(
            ws_row: UInt16(rows),
            ws_col: UInt16(cols),
            ws_xpixel: 0,
            ws_ypixel: 0
        )
        _ = ioctl(master, TIOCSWINSZ, &window)
        _ = kill(childPid, SIGWINCH)
    }

    func terminate() {
        _ = kill(childPid, SIGTERM)
    }

    private func setupReadSource() {
        _ = fcntl(master, F_SETFL, O_NONBLOCK)

        let source = DispatchSource.makeReadSource(fileDescriptor: master, queue: queue)
        source.setEventHandler { [weak self] in
            self?.readAvailable()
        }
        source.setCancelHandler { [master] in
            _ = Darwin.close(master)
        }
        source.resume()
        readSource = source
    }

    private func readAvailable() {
        var buffer = [UInt8](repeating: 0, count: 4096)
        while true {
            let bytesRead = Darwin.read(master, &buffer, buffer.count)
            if bytesRead > 0 {
                onData?(Data(buffer[0..<bytesRead]))
            } else if bytesRead == 0 {
                onExit?()
                readSource?.cancel()
                break
            } else if errno == EAGAIN || errno == EWOULDBLOCK {
                break
            } else if errno != EINTR {
                onExit?()
                readSource?.cancel()
                break
            }
        }
    }

    private static func execCommand(_ command: [String]) -> Never {
        var argv = command.map { strdup($0) }
        argv.append(nil)
        execvp(argv[0], &argv)
        let execErrno = errno
        for ptr in argv where ptr != nil {
            free(ptr)
        }
        fputs("exec failed: \(String(cString: strerror(execErrno)))\n", stderr)
        _exit(1)
    }
}
#endif
