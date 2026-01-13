/**
 * TTShell.initWithAction:target:profile:controller:customShell:
 *         commandAsShell:workingDirectory:sessionClass:restoreSession:provenance:
 *
 * Decompiled from Terminal.app using radare2 + r2ghidra
 * Address: 0x100034b00
 *
 * Initializes a new shell session with PTY.
 * Sets up terminal type, locale, and environment before fork.
 */

int64_t method_TTShell_initWithAction_target_profile_controller_customShell_commandAsShell_workingDirectory_sessionClass_restoreSession_provenance_(
    ulong self,
    uchar noname_1[16],
    int64_t action,
    ulong target,
    ulong profile,
    ulong controller,
    ulong customShell,
    ulong commandAsShell,
    ulong workingDirectory
)
{
    int tgetentResult;
    char *termType;
    char *fallbackTerm;
    uint32_t encoding;
    ulong localeIdentifier;
    ulong languageCode;
    ulong countryCode;
    int64_t shellObj;
    char termBuffer[128];

    // Stack canary for security
    int64_t stackGuard = *__stack_chk_guard;

    // Call super init
    shellObj = objc_msgSendSuper2(noname_1, /* selector: init */);

    if (shellObj == 0) {
        goto exit;
    }

    // Initialize file descriptors to invalid
    *(shellObj + 0x28) = 0xffffffffffffffff;  // masterFD = -1
    *(shellObj + 0x8a0) = workingDirectory;
    *(shellObj + 0x38) = 0xffffffff;          // Another FD
    *(shellObj + 0x48) = 0;
    *(shellObj + 0x4c) = 0x101;               // Some flags
    *(shellObj + 0x8bc) = 0;

    // Store references
    *(shellObj + 0x18) = target;

    if (action == 0) {
        action = 0;  // NULL check
    }
    *(shellObj + 0x20) = action;
    *(shellObj + 0x8) = controller;

    // Setup profile
    fcn_100090d20(shellObj, /* args */, profile);

    // Get application delegate
    fcn_100089240(*NSApp);
    fcn_10008b600();
    ulong appString = objc_autorelease(/* result */);

    // Setup initial environment string
    fcn_1000909e0(appString, "", "");

    // Get bundle path for environment
    fcn_10008acc0(NSBundle);  // [NSBundle mainBundle]
    fcn_10008bac0(/* bundle */, "");

    int64_t bundlePath = 0x1000c6488;  // Default if nil
    if (/* result */ != 0) {
        bundlePath = /* actual bundle path */;
    }
    fcn_1000909e0(appString, bundlePath, "");

    // Determine terminal type from profile
    ulong termValue = fcn_1000947a0(profile, /* key for terminalType */);
    int64_t termString = fcn_10008a600();

    if (termString == 0) {
        // No terminal type in profile, use default
        fcn_10008b620(profile);
        NSLog(@"No terminal type specified");

default_term:
        uint64_t fallbackIndex = 0;

try_fallback:
        // Fallback terminal types table at 0x1000c52b0
        // Contains: "xterm-256color", "xterm-color", "xterm", "vt220", "vt100", "dumb"
        int64_t offset = fallbackIndex << 3;

        do {
            fallbackTerm = *(offset + 0x1000c52b0);

            // Try to load terminfo for this terminal type
            tgetentResult = tgetent(NULL, fallbackTerm);

            fcn_10008b620(profile);

            if (0 < tgetentResult) {
                // Found a working terminal type
                NSLog(@"Using fallback terminal type");
                termType = fallbackTerm;
                goto set_term;
            }

            NSLog(@"Terminal type %s not found in terminfo", fallbackTerm);
            offset = offset + 8;

        } while (offset != 0x30);  // 6 entries * 8 bytes

        // None of the fallbacks worked
        fcn_10008b620(profile);
        NSLog(@"No valid terminal type found, using 'unknown'");
        termType = "unknown";

set_term:
        // Create NSString from terminal type
        fcn_1000930c0(*NSString, termType);
        fcn_1000909e0(appString, /* term string */, "");
    } else {
        // Terminal type specified in profile
        termType = fcn_100082560(termValue);  // Get C string

        // Verify it exists in terminfo
        tgetentResult = tgetent(NULL, termType);

        if (tgetentResult < 1) {
            // Specified terminal type not found
            fcn_10008b620(profile);
            NSLog(@"Specified terminal type %s not found", termType);

            if (termType == NULL) {
                goto default_term;
            }

            // If specified type was xterm-256color, try others
            int cmp = strcmp(termType, "xterm-256color");
            fallbackIndex = (cmp == 0) ? 1 : 0;  // Skip first if was 256color
            goto try_fallback;
        }

        if (termType != NULL) {
            goto set_term;
        }
    }

    // Setup locale-related environment variables
    fcn_1000947a0(*(shellObj + 0x10), /* key */);
    int hasLocale = fcn_100084400();

    if (hasLocale != 0) {
        // Get system locale
        fcn_1000947a0(*(shellObj + 0x10), /* key */);

        // Get encoding from profile
        encoding = fcn_1000941a0();
        ulong cfEncoding = CFStringConvertNSStringEncodingToEncoding(encoding);

        // Get current locale
        fcn_100085ce0(NSLocale);  // [NSLocale currentLocale]
        ulong locale = /* result */;

        // Get locale identifier (e.g., "en_US")
        localeIdentifier = *NSLocaleIdentifier;
        fcn_10008bae0(locale, localeIdentifier);

        // Build LC_CTYPE string (e.g., "en_US.UTF-8")
        fcn_10008a8c0(/* encoder */, cfEncoding);
        fcn_100093080(*NSString, "");

        // Get language code (e.g., "en")
        languageCode = *NSLocaleLanguageCode;
        fcn_10008bae0(locale, languageCode);

        // Get country code (e.g., "US")
        countryCode = *NSLocaleCountryCode;
        fcn_10008bae0(locale, countryCode);

        // Build LANG environment variable
        fcn_10008a8c0(/* encoder */, cfEncoding);
        fcn_100093080(*NSString, "");

        // Set LC_ALL, LANG, etc.
        fcn_10008bae0(locale, localeIdentifier);
        fcn_10008bae0(locale, languageCode);
        fcn_10008bae0(locale, countryCode);

        // ... additional locale setup ...
    }

    // Continue with shell command setup
    // ... (truncated - handles customShell, commandAsShell, etc.)

exit:
    // Verify stack canary
    if (stackGuard != *__stack_chk_guard) {
        __stack_chk_fail();
    }

    return shellObj;
}

/*
 * TTShell Object Layout (partial):
 *
 * Offset  Field
 * 0x08    controller
 * 0x10    profile
 * 0x18    target
 * 0x20    action
 * 0x28    masterFD (PTY master file descriptor)
 * 0x38    secondaryFD
 * 0x48    flags
 * 0x4c    moreFlags
 * 0x8a0   workingDirectory
 * 0x8bc   status
 *
 *
 * Terminal Type Fallback Order:
 *
 * 1. xterm-256color (modern, full color)
 * 2. xterm-color (basic color)
 * 3. xterm (no color)
 * 4. vt220 (DEC terminal)
 * 5. vt100 (basic DEC)
 * 6. dumb (minimal)
 *
 *
 * Environment Variables Set:
 *
 * - TERM: Terminal type from terminfo
 * - LANG: Locale with encoding (e.g., en_US.UTF-8)
 * - LC_CTYPE: Character encoding
 * - LC_ALL: May be set to locale
 * - Apple_PubSub_Socket_Render: For notifications
 * - TERM_PROGRAM: "Apple_Terminal"
 * - TERM_PROGRAM_VERSION: Terminal version
 *
 *
 * Key observations:
 *
 * 1. Uses tgetent() from libncurses for terminfo lookup
 * 2. Graceful fallback through terminal types
 * 3. Careful locale/encoding setup for international users
 * 4. Stack canary used for security
 * 5. File descriptors initialized to -1 (invalid)
 *
 *
 * PTY creation (done later in spawn method):
 *
 * The actual forkpty() call happens in a separate method.
 * This init just prepares the environment and settings.
 */

/*
 * Related method: TTShell.ptyPathNSString (0x10002aed4)
 *
 * Returns the path to the PTY slave device (e.g., "/dev/ttys001")
 * Used for job control and process identification.
 */
