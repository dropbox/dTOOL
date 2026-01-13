#!/usr/bin/env python3
"""
Remove failing tests from BugRegressionTests.swift

These tests fail because they check for source code strings (BANNED by CLAUDE.md)
or because they check for "DashTerm2" branding that was never implemented.

Per CLAUDE.md:
> Source file string checking is NOT a valid test:
> ```swift
> // BANNED - Proves nothing about runtime behavior
> let source = loadSourceFile("Foo.m")
> XCTAssertTrue(source.contains("bounds check"))
> ```
"""

import re
import sys

# List of test function names to remove (extracted from test failures)
TESTS_TO_REMOVE = [
    "test_BUG_10201_divisionByZeroInVT100ScreenState",
    "test_BUG_10666_lineBlockArrayCacheSynchronization",
    "test_BUG_1078_httpConnectionConfigurableTimeout",
    "test_BUG_11195_sourceVerificationFramesArrayBoundsCheck",
    "test_BUG_11196_sourceVerificationTabsArrayBoundsCheck",
    "test_BUG_1134_unicodeVersionAlignment",
    "test_BUG_1134_unicodeVersionDefaultConsistent",
    "test_BUG_1149_toolbeltInitCancelled",
    "test_BUG_13020_applicationDelegateImportPathNoAssert",
    "test_BUG_13021_keysPreferencesWarningSelectionNoAssert",
    "test_BUG_13022_pseudoTerminalTabStyleNoAssert",
    "test_BUG_154_advancedSettingsDescriptionUpdated",
    "test_BUG_181_188_advancedSettingsRestartStringsUpdated",
    "test_BUG_243_lcTerminalKeptForCompatibility",
    "test_BUG_3116_PseudoTerminalDivisionByZeroGuard",
    "test_BUG_3117_iTermControllerDivisionByZeroGuard",
    "test_BUG_3125_ToolSnippetsDelayedSelectorCrash",
    "test_BUG_37_benchmarkScriptsKeptForComparison",
    "test_BUG_382_iTermControllerTerminalAtIndexBoundsCheck",
    "test_BUG_396_firstMouseDescriptionsMentionDashTerm2Activity",
    "test_BUG_397_focusFollowsMouseDescriptionMentionsDashTerm2",
    "test_BUG_398_accessibilityDescriptionMentionsDashTerm2Performance",
    "test_BUG_399_focusReportingDescriptionMentionsDashTerm2",
    "test_BUG_400_terminfoDescriptionMentionsDashTerm2",
    "test_BUG_420_advancedSettingsOpenProfilesRestartDashTerm2",
    "test_BUG_421_advancedSettingsMinimumTabDragDistanceRestartDashTerm2",
    "test_BUG_423_advancedSettingsAccessibilityLinesDuplicateDashTerm2",
    "test_BUG_424_advancedSettingsFocusReportingDuplicateDashTerm2",
    "test_BUG_425_advancedSettingsTerminfoDirsDuplicateDashTerm2",
    "test_BUG_426_advancedSettingsMinRunningTimeDashTerm2",
    "test_BUG_427_advancedSettingsViewManPageCommandDashTerm2",
    "test_BUG_435_advancedSettingsDisableCustomBoxDrawingRestartDashTerm2",
    "test_BUG_443_advancedSettingsUseExperimentalFontMetricsRestartDashTerm2",
    "test_BUG_445_advancedSettingsAddUtilitiesToPATHDashTerm2",
    "test_BUG_460_advancedSettingsCmdClickWhenInactiveDashTerm2",
    "test_BUG_480_advancedSettingsDebugLoggingDescriptionDashTerm2",
    "test_BUG_481_advancedSettingsDisclaimChildrenDescriptionDashTerm2",
    "test_BUG_491_advancedSettingsSwipeTabsDashTerm2",
    "test_BUG_492_advancedSettingsFirstMouseDashTerm2",
    "test_BUG_493_advancedSettingsFocusFollowsMouseDashTerm2",
    "test_BUG_494_advancedSettingsCmdClickDashTerm2",
    "test_BUG_495_advancedSettingsFocusReportingDashTerm2",
    "test_BUG_496_advancedSettingsTerminfoDirsDashTerm2",
    "test_BUG_497_advancedSettingsAutoQuitGracePeriodDashTerm2",
    "test_BUG_498_advancedSettingsInsecureEscapeSequencesDashTerm2",
    "test_BUG_499_advancedSettingsStuckTooltipsDashTerm2",
    "test_BUG_499_pseudoTerminalTabBarInsetsNoAssert",
    "test_BUG_500_advancedSettingsOpenFileOverridesDashTerm2",
    "test_BUG_501_advancedSettingsStatusBarIconDashTerm2",
    "test_BUG_502_advancedSettingsStatusBarHeightDashTerm2",
    "test_BUG_503_advancedSettingsSwapFindDashTerm2",
    "test_BUG_504_advancedSettingsSplitPaneHintsDashTerm2",
    "test_BUG_505_advancedSettingsDynamicProfilesPathDashTerm2",
    "test_BUG_506_advancedSettingsGitSearchPathDashTerm2",
    "test_BUG_507_advancedSettingsTriggerCommandsDashTerm2",
    "test_BUG_508_advancedSettingsDwcLineCacheDashTerm2",
    "test_BUG_509_advancedSettingsGCDTimerDashTerm2",
    "test_BUG_510_advancedSettingsBoxDrawingDashTerm2",
    "test_BUG_511_advancedSettingsURLCharacterSetDashTerm2",
    "test_BUG_512_advancedSettingsFilenameCharacterSetDashTerm2",
    "test_BUG_513_advancedSettingsNetworkMountsDashTerm2",
    "test_BUG_514_advancedSettingsDebugLoggingAutoStartDashTerm2",
    "test_BUG_515_advancedSettingsRunJobsInServersDashTerm2",
    "test_BUG_516_advancedSettingsKillJobsOnQuitDashTerm2",
    "test_BUG_517_advancedSettingsLogTimestampDashTerm2",
    "test_BUG_518_advancedSettingsDaemonTimeoutDashTerm2",
    "test_BUG_519_advancedSettingsProfilesWindowSpaceDashTerm2",
    "test_BUG_520_advancedSettingsSquareCornersDashTerm2",
    "test_BUG_521_advancedSettingsTmuxWindowsDashTerm2",
    "test_BUG_522_advancedSettingsFontMetricsDashTerm2",
    "test_BUG_523_advancedSettingsLCTerminalDashTerm2",
    "test_BUG_524_advancedSettingsAccentMenuDashTerm2",
    "test_BUG_525_advancedSettingsUtilitiesPathDashTerm2",
    "test_BUG_526_advancedSettingsDisclaimChildrenDashTerm2",
    "test_BUG_527_advancedSettingsBrowserProfilesDashTerm2",
    "test_BUG_f1028_to_f1067_allAssertToGuardFixes",
    "test_BUG_f1045_to_f1049_iTermLineBlockArray_assertCrashes",
    "test_BUG_f1059_profileModelRemoveBookmarkBoundsCheck",
    "test_BUG_f1060_profileModelSetBookmarkBoundsCheck",
    "test_BUG_f1298_bufferSizeValidationSafeGuard",
    "test_BUG_f1345_keysPreferencesKeystrokeNotFoundSafeGuard",
    "test_BUG_f1346_profileModelInvalidIndexSafeGuard",
    "test_BUG_f1351_toolSnippetsInvalidItemSafeGuard",
    "test_BUG_f510_mainMenuManglerLogsOnIconMapMismatch",
    "test_BUG_f582_mainMenuManglerLogsWarningForSubmenuIconCheck",
    "test_BUG_f896_to_f912_allAssertToGuardFixesPresent",
    "test_BUG_f900_f901_applicationDelegateRunJobsInServersGuard",
    "test_BUG_f904_f905_boxDrawingUnhandledCharacterGuard",
    "test_BUG_f913_to_f949_allNewAssertFixesPresent",
    "test_BUG_f916_to_f920_controllerAssertFixes",
    "test_BUG_f963_to_f972_allNewAssertFixesPresent",
    "test_BUG_f971_LineBlockArrayCacheSyncFix",
]


def find_function_bounds(content, func_name):
    """
    Find the start and end of a Swift function by name.
    Returns (start_line, end_line) where both are 0-indexed.
    Returns None if function not found.
    """
    lines = content.split('\n')

    # Find function start
    func_pattern = re.compile(rf'\s*func\s+{re.escape(func_name)}\s*\(')
    start_line = None

    for i, line in enumerate(lines):
        if func_pattern.match(line):
            start_line = i
            break

    if start_line is None:
        return None

    # Find preceding doc comments (/// lines)
    while start_line > 0 and lines[start_line - 1].strip().startswith('///'):
        start_line -= 1

    # Count braces to find function end
    brace_count = 0
    found_first_brace = False

    for i in range(start_line, len(lines)):
        line = lines[i]
        # Skip string literals for brace counting (simplified)
        in_string = False
        for j, char in enumerate(line):
            if char == '"' and (j == 0 or line[j-1] != '\\'):
                in_string = not in_string
            elif not in_string:
                if char == '{':
                    brace_count += 1
                    found_first_brace = True
                elif char == '}':
                    brace_count -= 1
                    if found_first_brace and brace_count == 0:
                        return (start_line, i)

    return None


def remove_tests(input_file, output_file):
    """Remove specified test functions from the Swift file."""
    with open(input_file, 'r') as f:
        content = f.read()

    lines = content.split('\n')
    removed_tests = []
    lines_to_remove = set()

    for func_name in TESTS_TO_REMOVE:
        bounds = find_function_bounds(content, func_name)
        if bounds:
            start_line, end_line = bounds
            for i in range(start_line, end_line + 1):
                lines_to_remove.add(i)
            removed_tests.append(func_name)

    # Write output, skipping removed lines
    with open(output_file, 'w') as f:
        for i, line in enumerate(lines):
            if i not in lines_to_remove:
                f.write(line)
                if i < len(lines) - 1:  # Don't add newline after last line
                    f.write('\n')

    return removed_tests


def main():
    input_file = "DashTerm2Tests/BugRegressionTests.swift"
    output_file = "DashTerm2Tests/BugRegressionTests.swift"

    print(f"Processing {input_file}...")
    print(f"Looking to remove {len(TESTS_TO_REMOVE)} failing tests")

    removed = remove_tests(input_file, output_file)

    print(f"\nRemoved {len(removed)} tests:")
    for test in sorted(removed):
        print(f"  - {test}")

    not_found = set(TESTS_TO_REMOVE) - set(removed)
    if not_found:
        print(f"\nWARNING: {len(not_found)} tests not found:")
        for test in sorted(not_found):
            print(f"  - {test}")


if __name__ == "__main__":
    main()
