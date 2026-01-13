/**
 * TTView.drawRect:
 *
 * Decompiled from Terminal.app using radare2 + r2ghidra
 * Address: 0x100018e1c
 *
 * Main rendering entry point. Called by AppKit when view needs redraw.
 * Uses CoreGraphics/CoreText for CPU-based rendering (no GPU).
 */

void method_TTView_drawRect_(int64_t self, CGRect dirtyRect)
{
    ulong *pColorComponents;
    uint8_t isTextBlinkActive;
    uint8_t isCursorBlinkActive;
    uint8_t isVisualBellActive;
    uint8_t drawingToScreen;
    int isKeyWindow;
    uint32_t antialias;
    ulong scaleFactor;
    uint64_t cgContext;
    int64_t profile;
    int64_t rectCount;
    CGRect *dirtyRects;
    CGFloat backgroundAlpha;
    CGFloat red, green, blue, alpha;

    // Save blink states
    int64_t textBlinkOffset = _field_class_TTView_var__isTextBlinkActive;
    isTextBlinkActive = *(self + textBlinkOffset);

    int64_t cursorBlinkOffset = _field_class_TTView_var__isCursorBlinkActive;
    isCursorBlinkActive = *(self + cursorBlinkOffset);

    int64_t visualBellOffset = _field_class_TTView_var__isVisualBellActive;
    isVisualBellActive = *(self + visualBellOffset);

    // Store dirty rect
    CGRect savedRect;
    savedRect.origin.x = dirtyRect.origin.x;
    savedRect.origin.y = dirtyRect.origin.y;
    savedRect.size.width = dirtyRect.size.width;
    savedRect.size.height = dirtyRect.size.height;

    // Check if drawing to screen (vs printing/PDF)
    drawingToScreen = fcn_100089b20();

    // Get CoreGraphics context from NSGraphicsContext
    fcn_100085c20(NSGraphicsContext);  // [NSGraphicsContext currentContext]
    cgContext = fcn_100088240();        // .CGContext

    // Setup text matrix to identity
    CGAffineTransform identity = CGAffineTransformIdentity;
    CGContextSetTextMatrix(cgContext, &identity);

    // Set text drawing mode to fill (mode 0)
    CGContextSetTextDrawingMode(cgContext, 0);

    // Get profile for settings
    int64_t profileOffset = _field_class_TTView_var__profile;
    profile = *(self + profileOffset);

    // Check font antialiasing preference
    fcn_1000947a0(profile, /* key for fontAntialiased */);
    uint64_t shouldAntialias = fcn_100084400();

    if ((shouldAntialias & 1) == 0) {
        // Antialiasing disabled in preferences
        fcn_100094b40(self);
        double scale = fcn_100084000();  // Get scale factor

        scaleFactor = 0x3ff8000000000000;  // 1.5 as double

        if (1.5 < scale) {
            goto enable_antialiasing;
        }

        // Disable antialiasing for non-retina or low scale
        CGContextSetAllowsAntialiasing(cgContext, 0);
        antialias = false;
    } else {
enable_antialiasing:
        antialias = true;
    }

    // Check if this is key window (for selection styling)
    isKeyWindow = fcn_100085c40(NSGraphicsContext);
    uint8_t showSelection;
    if (isKeyWindow == 0) {
        showSelection = 0;
    } else {
        // Show selection if not currently dragging
        int64_t draggingOffset = _field_class_TTView_var__isDraggingSelection;
        showSelection = *(self + draggingOffset) ^ 1;
    }

    // Get background and foreground colors from profile
    fcn_1000947a0(profile, /* key for backgroundColor */);
    uint64_t bgColor = fcn_1000947a0(profile, /* key for foregroundColor */);

    // Determine which color to use based on key window state
    int64_t activeColor;
    isKeyWindow = fcn_100089da0(self);  // Check if key window
    if (isKeyWindow == 0) {
        activeColor = bgColor;
    } else {
        activeColor = /* foreground color */;
    }

    // Convert color to calibrated RGB colorspace
    CGFloat components[4] = {0.0, 0.0, 0.0, 0.0};
    fcn_100084e80(activeColor, NSCalibratedRGBColorSpace);
    fcn_100088160(/* color */, &components[0], &components[1],
                  &components[2], &components[3]);

    // Handle grayscale colors
    if (components[2] == 0.0) {  // No blue = might be grayscale
        if (isKeyWindow == 0) {
            activeColor = bgColor;
        }
        fcn_100084e80(activeColor, NSCalibratedRGBColorSpace);
        fcn_100088160(/* color */, &components[0], &components[1], NULL, NULL);
    }

    // Branch based on drawing mode
    if ((drawingToScreen & 1) == 0) {
        // Not drawing to screen (printing?)
        if ((showSelection & 1) == 0) {
            // Reset blink states
            *(self + textBlinkOffset) = 0;
            *(self + cursorBlinkOffset) = 0;
            *(self + visualBellOffset) = 0;

            // Get selection type
            fcn_10008c8a0(self);
            uint64_t selectionType = /* result */;

            if (selectionType == 2) {
                // Rectangular selection
                fcn_1000841a0(self, /* args */,
                    *(self + _field_class_TTView_var__textSelectionRanges));
                fcn_100083160();
            }
        }
    } else {
        // Drawing to screen - main path

        int64_t thumbnailOffset = _field_class_TTView_var__isDrawingThumbnail;

        // Determine background alpha
        if (*(self + thumbnailOffset) == '\x01') {
            // Drawing thumbnail - get alpha from color
            backgroundAlpha = fcn_1000837e0(activeColor);
        } else {
            // Normal drawing - use configured alpha
            backgroundAlpha = *(self + _field_class_TTView_var__backgroundColorAlpha);
        }

        // Clamp alpha to minimum (fully transparent causes issues)
        CGFloat minAlpha = 0.003;  // From 0x10009d720
        if (backgroundAlpha <= minAlpha) {
            backgroundAlpha = minAlpha;
        }

        // Create color with alpha for background
        fcn_100084ea0(backgroundAlpha, activeColor);
        fcn_10008eb40();  // Set as fill color

        // Check if background image is enabled
        fcn_1000947a0(profile, /* key for backgroundImagePath */);
        uint64_t hasBackgroundImage = /* result */;

        if (hasBackgroundImage == 0) {
            goto fill_background;
        }

        // ... handle background image ...

fill_background:
        // Get dirty rectangles from AppKit
        rectCount = 0;
        dirtyRects = NULL;

        if (((*(self + thumbnailOffset) & 1) == 0) &&
            (*(self + _field_class_TTView_var__isDraggingSelection) != '\x01'))
        {
            // Normal case: get actual dirty rects
            fcn_1000881a0(self, &dirtyRects, &rectCount);

            if (rectCount < 1) {
                goto draw_text;
            }
        } else {
            // Thumbnail or dragging: redraw entire rect
            rectCount = 1;
            dirtyRects = &savedRect;
        }

        // Fill background for each dirty rectangle
        int64_t rectIndex = 0;
        do {
            CGRect *currentRect = dirtyRects + rectIndex;

            // NSRectFillUsingOperation - CPU compositing operation
            // This is where GPU rendering would be much faster
            NSRectFillUsingOperation(
                currentRect->origin.x,
                currentRect->origin.y,
                currentRect->size.width,
                currentRect->size.height,
                1  // NSCompositingOperationCopy
            );

            rectIndex++;
        } while (rectIndex < rectCount);

draw_text:
        // Draw the actual text content
        // This calls into the line-by-line text rendering
        drawAttributedStringsToScreen(self, cgContext, savedRect,
            /* selections */, /* selectionColor */);
    }

    // Restore state and return
    // ... cleanup ...
}

/*
 * Key observations:
 *
 * 1. NO GPU RENDERING - uses CGContext (CoreGraphics) throughout
 *
 * 2. NSRectFillUsingOperation is CPU compositing:
 *    - Each dirty rect filled separately
 *    - No batching or GPU acceleration
 *
 * 3. CGContextSetTextMatrix sets up for CoreText rendering
 *
 * 4. Dirty rect optimization:
 *    - AppKit provides list of rects that need redraw
 *    - Only those regions are filled/drawn
 *    - But still CPU-based
 *
 * 5. Text drawing delegated to drawAttributedStringsToScreen:
 *    - Creates NSAttributedString per line
 *    - Uses CoreText for glyph rendering
 *    - No texture atlas or glyph caching
 *
 * 6. Alpha handling:
 *    - Minimum alpha of 0.003 enforced
 *    - Background alpha configurable per profile
 *
 * 7. Selection and blink states managed per-draw:
 *    - States reset during print/thumbnail
 *    - Allows different rendering for screen vs print
 *
 *
 * For dTerm improvement:
 *
 * Replace this entire flow with:
 * 1. Build vertex buffer from dirty cells
 * 2. Single GPU draw call (instanced)
 * 3. Use texture atlas for glyph lookup
 * 4. Render on dedicated thread
 */
