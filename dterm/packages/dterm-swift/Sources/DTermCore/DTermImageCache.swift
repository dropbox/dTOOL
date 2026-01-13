/*
 * DTermImageCache.swift - GPU Image Texture Cache for platform integration
 *
 * Copyright 2024-2025 Dropbox, Inc.
 * Licensed under Apache 2.0
 *
 * This module provides Swift bindings for the dterm-core image texture cache,
 * supporting Sixel, Kitty graphics protocol, and iTerm2 image protocol.
 *
 * ## Features
 *
 * - LRU cache with configurable memory budget (default 64MB)
 * - Handles RGBA, RGB, and ARGB (Sixel) formats
 * - Supports negative row indices for scrollback placement
 * - Automatic texture eviction when budget is exceeded
 *
 * ## Usage
 *
 * ```swift
 * let cache = DTermImageCache(memoryBudget: 64 * 1024 * 1024)
 *
 * // Upload an image
 * let handle = cache.upload(data: imageData, width: 100, height: 100, format: .rgba)
 *
 * // Place the image in the terminal grid
 * cache.place(handle: handle, row: 5, col: 10, widthCells: 4, heightCells: 3)
 *
 * // Later, remove the image
 * cache.remove(handle: handle)
 * ```
 */

import Foundation
import CDTermCore

// MARK: - Image Format

/// Image pixel format for upload.
public enum ImageFormat: UInt8 {
    /// RGBA (4 bytes per pixel, R-G-B-A order).
    case rgba = 0
    /// RGB (3 bytes per pixel, R-G-B order).
    case rgb = 1
    /// ARGB (4 bytes per pixel, A-R-G-B order - Sixel format).
    case argb = 2

    /// Bytes per pixel for this format.
    public var bytesPerPixel: Int {
        switch self {
        case .rgba, .argb: return 4
        case .rgb: return 3
        }
    }
}

// MARK: - Image Handle

/// Handle to a GPU image texture.
///
/// Handles are unique within a cache instance and are never reused.
/// A handle value of 0 indicates a null/invalid handle.
public struct ImageHandle: Equatable, Hashable {
    /// Raw handle value.
    public let raw: UInt64

    /// Create a handle from a raw u64 value.
    public init(raw: UInt64) {
        self.raw = raw
    }

    /// A null/invalid handle.
    public static let null = ImageHandle(raw: 0)

    /// Check if this is a null handle.
    public var isNull: Bool { raw == 0 }
}

// MARK: - Image Placement

/// Placement of an image in the terminal grid.
public struct ImagePlacement {
    /// Handle to the image texture.
    public var handle: ImageHandle

    /// Row position (negative = scrollback).
    public var row: Int64

    /// Column position.
    public var col: UInt16

    /// Width in terminal cells.
    public var widthCells: UInt16

    /// Height in terminal cells.
    public var heightCells: UInt16

    /// Z-index for stacking (negative = below text).
    public var zIndex: Int32

    /// Create a new placement.
    public init(
        handle: ImageHandle,
        row: Int64,
        col: UInt16,
        widthCells: UInt16,
        heightCells: UInt16,
        zIndex: Int32 = 0
    ) {
        self.handle = handle
        self.row = row
        self.col = col
        self.widthCells = widthCells
        self.heightCells = heightCells
        self.zIndex = zIndex
    }
}

// MARK: - Image Texture Cache

/// GPU image texture cache with LRU eviction.
///
/// This cache manages GPU textures for inline images, automatically evicting
/// least-recently-used images when the memory budget is exceeded.
///
/// Thread Safety: This class is NOT thread-safe. Use external synchronization
/// if accessing from multiple threads.
public final class DTermImageCache {
    /// Handle to the native cache.
    private var handle: OpaquePointer?

    /// Default memory budget (64 MB).
    public static let defaultMemoryBudget: Int = 64 * 1024 * 1024

    /// Maximum image dimension in pixels.
    public static let maxImageDimension: UInt32 = 10000

    // MARK: - Initialization

    /// Create a new image texture cache.
    ///
    /// - Parameter memoryBudget: Maximum GPU memory budget in bytes.
    ///   Defaults to 64 MB if not specified or 0.
    public init(memoryBudget: Int = 0) {
        let budget = memoryBudget == 0 ? Self.defaultMemoryBudget : memoryBudget
        handle = dterm_image_cache_create(UInt(budget))
    }

    deinit {
        if let handle = handle {
            dterm_image_cache_free(handle)
        }
    }

    // MARK: - State

    /// The memory budget in bytes.
    public var memoryBudget: Int {
        // Note: There's no getter FFI, so we track internally if needed
        // For now, just return the default
        Self.defaultMemoryBudget
    }

    /// Current memory usage in bytes.
    public var memoryUsed: Int {
        guard let handle = handle else { return 0 }
        return Int(dterm_image_cache_memory_used(handle))
    }

    /// Number of stored images.
    public var imageCount: Int {
        guard let handle = handle else { return 0 }
        return Int(dterm_image_cache_image_count(handle))
    }

    /// Number of active placements.
    public var placementCount: Int {
        guard let handle = handle else { return 0 }
        return Int(dterm_image_cache_placement_count(handle))
    }

    /// Check if the image cache feature is available.
    public static var isAvailable: Bool {
        dterm_image_cache_available()
    }

    // MARK: - Memory Management

    /// Set the memory budget.
    ///
    /// If the new budget is lower than current usage, images will be evicted.
    ///
    /// - Parameter bytes: New memory budget in bytes.
    public func setMemoryBudget(_ bytes: Int) {
        guard let handle = handle else { return }
        dterm_image_cache_set_budget(handle, UInt(bytes))
    }

    /// Clear all images and placements.
    public func clear() {
        guard let handle = handle else { return }
        dterm_image_cache_clear(handle)
    }

    // MARK: - Image Upload

    /// Upload an image to the cache.
    ///
    /// This allocates a handle and converts the image data to RGBA format for GPU upload.
    /// The returned RGBA data should be uploaded to your GPU texture, then freed with
    /// `freeRgbaData(_:count:)`.
    ///
    /// - Parameters:
    ///   - data: Source image data in the specified format.
    ///   - width: Image width in pixels.
    ///   - height: Image height in pixels.
    ///   - format: Source pixel format.
    /// - Returns: A tuple of (handle, rgbaData, rgbaCount), or nil if upload failed.
    ///   The caller is responsible for freeing rgbaData using `freeRgbaData`.
    public func upload(
        data: Data,
        width: UInt32,
        height: UInt32,
        format: ImageFormat
    ) -> (handle: ImageHandle, rgbaData: UnsafeMutablePointer<UInt8>, rgbaCount: Int)? {
        guard let handle = handle else { return nil }

        // Validate dimensions
        guard width > 0 && height > 0 &&
              width <= Self.maxImageDimension &&
              height <= Self.maxImageDimension else {
            return nil
        }

        // Validate data size
        let expectedSize = Int(width) * Int(height) * format.bytesPerPixel
        guard data.count >= expectedSize else { return nil }

        var outRgba: UnsafeMutablePointer<UInt8>? = nil
        var outLen: UInt = 0

        let imageHandle = data.withUnsafeBytes { buffer -> UInt64 in
            guard let baseAddress = buffer.baseAddress else { return 0 }
            let ptr = baseAddress.assumingMemoryBound(to: UInt8.self)

            return dterm_image_cache_upload(
                handle,
                ptr,
                UInt(data.count),
                width,
                height,
                format.rawValue,
                &outRgba,
                &outLen
            )
        }

        guard imageHandle != 0, let rgbaPtr = outRgba, outLen > 0 else {
            return nil
        }

        return (
            handle: ImageHandle(raw: imageHandle),
            rgbaData: rgbaPtr,
            rgbaCount: Int(outLen)
        )
    }

    /// Free RGBA data returned by `upload`.
    ///
    /// - Parameters:
    ///   - data: The rgbaData pointer from upload.
    ///   - count: The rgbaCount from upload.
    public func freeRgbaData(_ data: UnsafeMutablePointer<UInt8>, count: Int) {
        dterm_image_free_rgba(data, UInt(count))
    }

    /// Convenience method to upload an image and get just the handle.
    ///
    /// This handles converting and freeing the RGBA data internally.
    /// Use `upload(data:width:height:format:)` if you need the RGBA data
    /// to upload to your own GPU texture.
    ///
    /// - Parameters:
    ///   - data: Source image data in the specified format.
    ///   - width: Image width in pixels.
    ///   - height: Image height in pixels.
    ///   - format: Source pixel format.
    ///   - uploadToGpu: Closure that receives the RGBA data for GPU upload.
    /// - Returns: An image handle, or `ImageHandle.null` if upload failed.
    public func uploadAndConvert(
        data: Data,
        width: UInt32,
        height: UInt32,
        format: ImageFormat,
        uploadToGpu: (UnsafePointer<UInt8>, Int) -> Void
    ) -> ImageHandle {
        guard let result = upload(data: data, width: width, height: height, format: format) else {
            return .null
        }

        // Let the caller upload to GPU
        uploadToGpu(result.rgbaData, result.rgbaCount)

        // Free the RGBA data
        freeRgbaData(result.rgbaData, count: result.rgbaCount)

        return result.handle
    }

    // MARK: - Image Removal

    /// Remove an image and free its memory.
    ///
    /// Note: The caller is responsible for freeing the actual GPU texture.
    ///
    /// - Parameter handle: Handle to remove.
    /// - Returns: `true` if the image was found and removed.
    @discardableResult
    public func remove(handle: ImageHandle) -> Bool {
        guard let cacheHandle = self.handle else { return false }
        return dterm_image_cache_remove(cacheHandle, handle.raw)
    }

    // MARK: - Image Placement

    /// Place an image at a terminal position.
    ///
    /// - Parameters:
    ///   - handle: Handle to the image.
    ///   - row: Row position (negative = scrollback).
    ///   - col: Column position.
    ///   - widthCells: Width in terminal cells.
    ///   - heightCells: Height in terminal cells.
    public func place(
        handle: ImageHandle,
        row: Int64,
        col: UInt16,
        widthCells: UInt16,
        heightCells: UInt16
    ) {
        guard let cacheHandle = self.handle else { return }
        dterm_image_cache_place(cacheHandle, handle.raw, row, col, widthCells, heightCells)
    }

    /// Place an image using an ImagePlacement struct.
    ///
    /// - Parameter placement: Placement configuration.
    public func place(_ placement: ImagePlacement) {
        place(
            handle: placement.handle,
            row: placement.row,
            col: placement.col,
            widthCells: placement.widthCells,
            heightCells: placement.heightCells
        )
    }
}
