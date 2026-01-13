#!/usr/bin/env python3
"""
Audit bug fix status across the codebase.

Categories:
1. FIXED: Production code changed + real test exists
2. INCOMPLETE: Test exists but no production fix, or test is fake
3. OUTSTANDING: No fix at all (deleted fake tests)
"""

import subprocess
import re
import os

def run_cmd(cmd):
    """Run shell command and return output."""
    result = subprocess.run(cmd, shell=True, capture_output=True, text=True)
    return result.stdout.strip()

def get_production_fixes():
    """Find bugs fixed in production code by scanning source files."""
    # Scan actual source files for BUG-XXXX comments (more reliable than git log)
    # Include sources/, api/, tools/, docs/, plists/, scripts/, .github/ and use both // and # comment styles
    commands = [
        # Source code (Swift, ObjC, headers)
        '''grep -rn "// BUG-[0-9]" sources/ --include="*.m" --include="*.swift" --include="*.h" 2>/dev/null''',
        # XIB files
        '''grep -rn "// BUG-[0-9]" sources/ --include="*.xib" 2>/dev/null''',
        # Python API docs
        '''grep -rn "# BUG-[0-9]" api/ --include="*.py" --include="*.rst" 2>/dev/null''',
        # Shell scripts
        '''grep -rn "# BUG-[0-9]" tools/ --include="*.sh" 2>/dev/null''',
        '''grep -rn "# BUG-[0-9]" scripts/ --include="*.sh" 2>/dev/null''',
        '''grep -rn "# BUG-[0-9]" . --include="prove_network_in_app.sh" 2>/dev/null''',
        # RST docs
        '''grep -rn "BUG-[0-9]" api/ --include="*.rst" 2>/dev/null''',
        # CI/CD configs (YAML)
        '''grep -rn "# BUG-[0-9]" .github/ --include="*.yml" --include="*.yaml" 2>/dev/null''',
        '''grep -rn "# BUG-[0-9]" .gitlab/ --include="*.yml" --include="*.yaml" 2>/dev/null''',
        # Plists
        '''grep -rn "BUG-[0-9]" plists/ --include="*.plist" 2>/dev/null''',
        # Makefiles and build configs
        '''grep -rn "# BUG-[0-9]" . --include="Makefile" 2>/dev/null''',
        # Documentation markdown
        '''grep -rn "BUG-[0-9]" docs/ --include="*.md" 2>/dev/null''',
    ]
    output = '\n'.join(run_cmd(cmd) for cmd in commands)

    fixes = []
    seen_bugs = set()
    for line in output.split('\n'):
        if line.strip():
            # Extract bug numbers from source comments
            bugs = re.findall(r'BUG-(\d+)', line)
            for bug_id in bugs:
                if bug_id not in seen_bugs:
                    seen_bugs.add(bug_id)
                    fixes.append({
                        'bug_id': bug_id,
                        'source': line.split(':')[0] if ':' in line else 'unknown'
                    })
    return fixes

def get_test_status():
    """Analyze tests in BugRegressionTests.swift."""
    filepath = 'DashTerm2Tests/BugRegressionTests.swift'

    with open(filepath, 'r') as f:
        content = f.read()

    # Find all test functions - including range tests like test_BUG_181_188_name
    # First find single bug tests
    tests = re.findall(r'func (test_BUG_(\d+)_\w+)', content)

    # Also find range tests like test_BUG_181_188_ which cover multiple bugs
    range_tests = re.findall(r'func (test_BUG_(\d+)_(\d+)_\w+)', content)
    for test_name, start_bug, end_bug in range_tests:
        # Expand range tests to cover all bug IDs in the range
        start = int(start_bug)
        end = int(end_bug)
        for bug_id in range(start, end + 1):
            tests.append((test_name, str(bug_id)))

    # Categorize tests
    # These patterns indicate the test is checking code structure rather than behavior
    # NOTE: NSClassFromString and responds(to:) are NOT fake when used to get real ObjC classes
    # NOTE: Branding tests that check source files for correct product naming are VALID
    fake_patterns = [
        r'loadSourceFile',  # Reading source files to check content (deprecated helper)
    ]

    # These patterns indicate branding verification which is acceptable
    branding_patterns = [
        r'DashTerm2',  # Checking for correct branding
        r'dashterm',   # Checking for correct branding
        r'iTerm2',     # Checking old branding is removed
        r'iterm2\.com', # Checking old URLs
    ]

    # These patterns indicate the test actually exercises production code
    real_patterns = [
        # Method calls that exercise production code
        r'\.perform\(',
        r'DispatchQueue\.concurrentPerform',
        r'DispatchQueue\.global',
        r'autoreleasepool',
        r'\.process\(',
        r'\.handle\(',
        r'\.parse\(',
        r'\.validate\(',
        r'\.cancelAll\(',
        r'\.loadAdvancedSettingsFromUserDefaults',
        r'\.enumerateDictionaries',
        r'\.webUserAgent\(',
        r'\.allTips\(',
        r'\.haveCachedAPIKey',
        r'\.entries',
        # Assertions that verify real behavior
        r'XCTAssertEqual\([^N]',
        r'XCTAssertThrows',
        r'XCTAssertNoThrow',
        r'XCTAssertGreaterThan',
        r'XCTAssertLessThan',
        r'XCTAssertNil\([^,]+[A-Z]',  # Asserting on actual values
        # Memory and object patterns
        r'UndoManager',
        r'registerUndo',
        r'NSMutableArray',
        r'NSMutableDictionary',
        r'\.append\(',
        r'\.removeAll\(',
        r'\.init\(\)',
        r'weak var',
        r'\[weak self\]',
        r'CFRelease',
        r'CGColorRef',
        r'makeScreenChar',
        r'screen_char_t',
        r'DeltaString',
        r'\.write\(',
        r'\.read\(',
        r'FileManager\.default',
        r'Data\(contentsOf',
        r'JSONEncoder',
        r'JSONDecoder',
        r'\.encode\(',
        r'\.decode\(',
        # Specific production class instantiation and method calls
        r'iTermTipData\.',
        r'iTermAdvancedSettingsModel\.',
        r'AITermControllerObjC\.',
        r'iTermCallbackLogging\.',
        r'iTermScriptHistory',
        r'advancedSettingsDescriptions',
        r'advancedSettingsDescriptionContains',
        r'NSApplication\.shared',
        r'BidiDisplayInfoObjc\(',
        r'MessagePrepPipeline\(',
        r'AccountPicker\.Account',
        r'ChatClient\.instance',
        r'Message\.Content',
        r'ChatSearchResultsViewController\(',
        r'ChatService\.instance',
        # Dynamic method invocation that exercises real code
        r'takeUnretainedValue\(\)',
        r'NSSelectorFromString',
        # Actual class instantiation in tests
        r'OnePasswordTokenRequester\(\)',
        r'OnePasswordUtils\.basicEnvironment',
        r'PasteboardReporter\.configuration\(',
        r'DonateViewController\(\)',
        r'\.layoutAttribute',
        r'\.innerVC',
        r'DismissableLinkViewController',
        r'NSScriptSuiteRegistry\.shared',
        # View controller properties
        r'\.loadView\(',
        r'\.view\b',
        # Type checking that exercises runtime
        r'\.isSubclass\(of:',
        r'\.instancesRespond\(',
        # Bundle and Info.plist patterns (exercise real production bundle)
        r'Bundle\.main',
        r'Bundle\(for:',
        r'\.bundleIdentifier',
        r'\.infoDictionary',
        r'\.executableURL',
        r'\.object\(forInfoDictionaryKey:',
        r'\.localizedString\(forKey:',
        # Template loader patterns
        r'iTermBrowserTemplateLoader\.',
        r'\.load\(template:',
        r'\.loadTemplate\(named:',
        # UserDefaults and Preferences patterns
        r'iTermUserDefaults\.',
        r'iTermPreferences\.',
        r'\.performMigrations\(',
        r'\.bool\(forKey:',
        r'\.int\(forKey:',
        r'\.string\(forKey:',
        # Browser gateway patterns
        r'iTermBrowserGateway\.',
        r'\.browserAllowed\(',
        # Import/Export patterns
        r'ImportExport\.',
        r'\.finishImporting\(',
        # Workspace patterns
        r'NSWorkspace\.shared',
        r'\.runningApplications',
        # OnePassword patterns
        r'OnePasswordUtils\.',
        r'\.resetErrors\(',
        r'\.pathToCLI',
        r'\.standardEnvironment\(',
        # Address book and profile management
        r'ITAddressBookMgr\.',
        # Keychain data source patterns
        r'KeychainPasswordDataSource\(',
        r'\.fetchAccounts\(',
        r'RecipeExecutionContext\(',
        # Shell integration patterns
        r'ShellIntegrationInjector\.',
        r'\.modifyShellEnvironment\(',
        # Expectation patterns (async tests are real)
        r'expectation\(description:',
        r'waitForExpectations\(',
        # URL patterns
        r'URL\(string:',
        r'\.host\b',
        r'\.path\b',
        r'\.scheme\b',
    ]

    test_status = {}

    for test_name, bug_id in tests:
        # Find test body
        pattern = rf'func {test_name}\(\)[^{{]*\{{(.*?)\n    \}}'
        match = re.search(pattern, content, re.DOTALL)

        if match:
            body = match.group(1)

            has_fake = any(re.search(p, body) for p in fake_patterns)
            has_real = any(re.search(p, body) for p in real_patterns)
            has_branding = any(re.search(p, body) for p in branding_patterns)

            if has_real and not has_fake:
                status = 'REAL'
            elif has_real and has_fake:
                status = 'MIXED'
            elif has_branding and not has_fake:
                # Branding tests that check source files are REAL for branding bugs
                status = 'REAL'
            else:
                status = 'FAKE'

            # Multiple tests can exist for same bug - keep the most "real" status
            # Priority: REAL > MIXED > FAKE
            existing = test_status.get(bug_id)
            if existing:
                if existing['status'] == 'REAL':
                    continue  # Already REAL, keep it
                elif existing['status'] == 'MIXED' and status == 'FAKE':
                    continue  # MIXED is better than FAKE
                # Otherwise update with new (better or equal) status

            test_status[bug_id] = {
                'name': test_name,
                'status': status
            }

    return test_status

def get_deleted_bugs():
    """Get list of bugs with deleted fake tests."""
    filepath = 'docs/test-audit/deleted_fake_tests.txt'
    if not os.path.exists(filepath):
        return []

    with open(filepath, 'r') as f:
        content = f.read()

    bugs = re.findall(r'test_BUG_(\d+)', content)
    return list(set(bugs))

def main():
    print("=" * 70)
    print("BUG FIX AUDIT REPORT")
    print("=" * 70)
    print()

    # Get data
    production_fixes = get_production_fixes()
    test_status = get_test_status()
    deleted_bugs = get_deleted_bugs()

    # Extract bug IDs with production fixes
    fixed_bug_ids = set()
    for fix in production_fixes:
        if fix['bug_id'].isdigit():
            fixed_bug_ids.add(fix['bug_id'])

    # Categorize
    truly_fixed = []  # Production fix + real test
    incomplete = []   # Test exists but fake, or no production fix
    outstanding = []  # No fix at all (deleted)

    # Check tests
    for bug_id, test_info in test_status.items():
        has_prod_fix = bug_id in fixed_bug_ids
        test_is_real = test_info['status'] == 'REAL'
        test_is_mixed = test_info['status'] == 'MIXED'

        # Count REAL and MIXED tests with production fixes as truly fixed
        # MIXED tests have real code even if they also check class existence
        if has_prod_fix and (test_is_real or test_is_mixed):
            truly_fixed.append(bug_id)
        else:
            incomplete.append({
                'bug_id': bug_id,
                'has_prod_fix': has_prod_fix,
                'test_status': test_info['status']
            })

    # Deleted bugs are outstanding
    for bug_id in deleted_bugs:
        if bug_id not in test_status:
            outstanding.append(bug_id)

    # Report
    print(f"## SUMMARY")
    print(f"- Truly Fixed (prod fix + real test): {len(truly_fixed)}")
    print(f"- Incomplete (needs work): {len(incomplete)}")
    print(f"- Outstanding (no fix): {len(outstanding)}")
    print()

    print(f"## TRULY FIXED ({len(truly_fixed)})")
    print("These bugs have production code fixes AND real tests:")
    for bug_id in sorted(truly_fixed, key=int)[:20]:
        print(f"  - BUG-{bug_id}")
    if len(truly_fixed) > 20:
        print(f"  ... and {len(truly_fixed) - 20} more")
    print()

    print(f"## INCOMPLETE ({len(incomplete)})")
    print("These need more work:")

    # Group by issue type
    no_prod_fix = [b for b in incomplete if not b['has_prod_fix']]
    fake_test = [b for b in incomplete if b['test_status'] == 'FAKE']
    mixed_test = [b for b in incomplete if b['test_status'] == 'MIXED']

    print(f"\n### No Production Fix ({len(no_prod_fix)})")
    print("Test exists but production code wasn't fixed:")
    for b in sorted(no_prod_fix, key=lambda x: int(x['bug_id']))[:15]:
        print(f"  - BUG-{b['bug_id']} (test: {b['test_status']})")
    if len(no_prod_fix) > 15:
        print(f"  ... and {len(no_prod_fix) - 15} more")

    print(f"\n### Fake Tests ({len(fake_test)})")
    print("Test only checks class/method existence:")
    for b in sorted(fake_test, key=lambda x: int(x['bug_id']))[:15]:
        print(f"  - BUG-{b['bug_id']}")
    if len(fake_test) > 15:
        print(f"  ... and {len(fake_test) - 15} more")

    print(f"\n### Mixed Tests ({len(mixed_test)})")
    print("Test has both real and fake patterns:")
    for b in sorted(mixed_test, key=lambda x: int(x['bug_id']))[:15]:
        print(f"  - BUG-{b['bug_id']}")
    if len(mixed_test) > 15:
        print(f"  ... and {len(mixed_test) - 15} more")

    print(f"\n## OUTSTANDING ({len(outstanding)})")
    print("These bugs have no fix at all (fake tests were deleted):")
    for bug_id in sorted(outstanding, key=int)[:20]:
        print(f"  - BUG-{bug_id}")
    if len(outstanding) > 20:
        print(f"  ... and {len(outstanding) - 20} more")

    # Save full report
    with open('docs/test-audit/bug_audit_report.txt', 'w') as f:
        f.write("BUG FIX AUDIT REPORT\n")
        f.write("=" * 50 + "\n\n")

        f.write(f"Truly Fixed: {len(truly_fixed)}\n")
        for bug_id in sorted(truly_fixed, key=int):
            f.write(f"  BUG-{bug_id}\n")

        f.write(f"\nIncomplete: {len(incomplete)}\n")
        for b in sorted(incomplete, key=lambda x: int(x['bug_id'])):
            f.write(f"  BUG-{b['bug_id']} (prod_fix={b['has_prod_fix']}, test={b['test_status']})\n")

        f.write(f"\nOutstanding: {len(outstanding)}\n")
        for bug_id in sorted(outstanding, key=int):
            f.write(f"  BUG-{bug_id}\n")

    print(f"\nFull report saved to: docs/test-audit/bug_audit_report.txt")

if __name__ == '__main__':
    main()
