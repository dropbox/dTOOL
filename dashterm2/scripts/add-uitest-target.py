#!/usr/bin/env python3
"""
Add DashTerm2UITests target to the Xcode project.

This script modifies the project.pbxproj file to add a UI test target.
It uses proper UUID generation and follows Xcode's pbxproj conventions.
"""

import re
import uuid
import sys
from pathlib import Path

def generate_xcode_uuid():
    """Generate a 24-character uppercase hex UUID like Xcode uses."""
    return uuid.uuid4().hex[:24].upper()

def add_uitest_target(project_path: Path):
    """Add the UI test target to the Xcode project."""

    pbxproj_path = project_path / "project.pbxproj"

    if not pbxproj_path.exists():
        print(f"Error: {pbxproj_path} not found")
        return False

    content = pbxproj_path.read_text()

    # Check if target already exists
    if "DashTerm2UITests" in content:
        print("UI test target already exists in project")
        return True

    # Generate UUIDs for all the objects we need to create
    uuids = {
        'file_ref_tests': generate_xcode_uuid(),
        'file_ref_info': generate_xcode_uuid(),
        'group': generate_xcode_uuid(),
        'build_file_tests': generate_xcode_uuid(),
        'sources_build_phase': generate_xcode_uuid(),
        'frameworks_build_phase': generate_xcode_uuid(),
        'resources_build_phase': generate_xcode_uuid(),
        'target_dependency': generate_xcode_uuid(),
        'container_item_proxy': generate_xcode_uuid(),
        'product_ref': generate_xcode_uuid(),
        'native_target': generate_xcode_uuid(),
        'build_config_debug': generate_xcode_uuid(),
        'build_config_release': generate_xcode_uuid(),
        'build_config_development': generate_xcode_uuid(),
        'build_config_nightly': generate_xcode_uuid(),
        'build_config_list': generate_xcode_uuid(),
    }

    # Find the main app target UUID
    main_target_match = re.search(r'(\w{24}) /\* DashTerm2 \*/ = \{[^}]*isa = PBXNativeTarget[^}]*productType = "com\.apple\.product-type\.application"', content)
    if not main_target_match:
        # Try alternative pattern
        main_target_match = re.search(r'(\w{24}) /\* DashTerm2 \*/ = \{\s*isa = PBXNativeTarget;', content)

    main_target_uuid = main_target_match.group(1) if main_target_match else None

    if not main_target_uuid:
        print("Warning: Could not find main DashTerm2 target UUID")
        # Use a fallback - search for the target by name
        target_match = re.search(r'(\w{24}) /\* DashTerm2 \*/ = \{', content)
        if target_match:
            main_target_uuid = target_match.group(1)

    print(f"Main target UUID: {main_target_uuid}")

    # 1. Add PBXFileReference entries
    file_ref_section = "/* Begin PBXFileReference section */"
    file_refs = f'''
		{uuids['file_ref_tests']} /* DashTerm2UITests.swift */ = {{isa = PBXFileReference; lastKnownFileType = sourcecode.swift; path = DashTerm2UITests.swift; sourceTree = "<group>"; }};
		{uuids['file_ref_info']} /* Info.plist */ = {{isa = PBXFileReference; lastKnownFileType = text.plist.xml; path = Info.plist; sourceTree = "<group>"; }};
		{uuids['product_ref']} /* DashTerm2UITests.xctest */ = {{isa = PBXFileReference; explicitFileType = wrapper.cfbundle; includeInIndex = 0; path = DashTerm2UITests.xctest; sourceTree = BUILT_PRODUCTS_DIR; }};'''

    content = content.replace(file_ref_section, file_ref_section + file_refs)

    # 2. Add PBXGroup for the test files
    # Find the main group and add our group there
    main_group_match = re.search(r'mainGroup = (\w{24})', content)
    main_group_uuid = main_group_match.group(1) if main_group_match else None

    group_section = "/* Begin PBXGroup section */"
    group_entry = f'''
		{uuids['group']} /* DashTerm2UITests */ = {{
			isa = PBXGroup;
			children = (
				{uuids['file_ref_tests']} /* DashTerm2UITests.swift */,
				{uuids['file_ref_info']} /* Info.plist */,
			);
			path = DashTerm2UITests;
			sourceTree = "<group>";
		}};'''

    content = content.replace(group_section, group_section + group_entry)

    # Add group to main group's children
    if main_group_uuid:
        # Find the main group and add our group to its children
        main_group_pattern = re.compile(
            rf'({main_group_uuid} /\* .* \*/ = \{{\s*isa = PBXGroup;\s*children = \()([^)]*)',
            re.MULTILINE | re.DOTALL
        )
        match = main_group_pattern.search(content)
        if match:
            children_start = match.group(1)
            children_content = match.group(2)
            new_children = f"{children_start}\n\t\t\t\t{uuids['group']} /* DashTerm2UITests */,{children_content}"
            content = main_group_pattern.sub(new_children, content)

    # 3. Add PBXBuildFile entry
    build_file_section = "/* Begin PBXBuildFile section */"
    build_file = f'''
		{uuids['build_file_tests']} /* DashTerm2UITests.swift in Sources */ = {{isa = PBXBuildFile; fileRef = {uuids['file_ref_tests']} /* DashTerm2UITests.swift */; }};'''

    content = content.replace(build_file_section, build_file_section + build_file)

    # 4. Add PBXSourcesBuildPhase
    sources_phase_section = "/* Begin PBXSourcesBuildPhase section */"
    sources_phase = f'''
		{uuids['sources_build_phase']} /* Sources */ = {{
			isa = PBXSourcesBuildPhase;
			buildActionMask = 2147483647;
			files = (
				{uuids['build_file_tests']} /* DashTerm2UITests.swift in Sources */,
			);
			runOnlyForDeploymentPostprocessing = 0;
		}};'''

    content = content.replace(sources_phase_section, sources_phase_section + sources_phase)

    # 5. Add PBXFrameworksBuildPhase
    frameworks_phase_section = "/* Begin PBXFrameworksBuildPhase section */"
    frameworks_phase = f'''
		{uuids['frameworks_build_phase']} /* Frameworks */ = {{
			isa = PBXFrameworksBuildPhase;
			buildActionMask = 2147483647;
			files = (
			);
			runOnlyForDeploymentPostprocessing = 0;
		}};'''

    content = content.replace(frameworks_phase_section, frameworks_phase_section + frameworks_phase)

    # 6. Add PBXResourcesBuildPhase
    resources_phase_section = "/* Begin PBXResourcesBuildPhase section */"
    resources_phase = f'''
		{uuids['resources_build_phase']} /* Resources */ = {{
			isa = PBXResourcesBuildPhase;
			buildActionMask = 2147483647;
			files = (
			);
			runOnlyForDeploymentPostprocessing = 0;
		}};'''

    content = content.replace(resources_phase_section, resources_phase_section + resources_phase)

    # 7. Add PBXContainerItemProxy and PBXTargetDependency
    if main_target_uuid:
        # Find the root object (project object) UUID
        root_match = re.search(r'rootObject = (\w+)', content)
        root_uuid = root_match.group(1) if root_match else "0464AB0C006CD2EC7F000001"

        container_proxy_section = "/* Begin PBXContainerItemProxy section */"
        if container_proxy_section in content:
            container_proxy = f'''
		{uuids['container_item_proxy']} /* PBXContainerItemProxy */ = {{
			isa = PBXContainerItemProxy;
			containerPortal = {root_uuid} /* Project object */;
			proxyType = 1;
			remoteGlobalIDString = {main_target_uuid};
			remoteInfo = DashTerm2;
		}};'''
            content = content.replace(container_proxy_section, container_proxy_section + container_proxy)

        target_dep_section = "/* Begin PBXTargetDependency section */"
        if target_dep_section in content:
            target_dep = f'''
		{uuids['target_dependency']} /* PBXTargetDependency */ = {{
			isa = PBXTargetDependency;
			target = {main_target_uuid} /* DashTerm2 */;
			targetProxy = {uuids['container_item_proxy']} /* PBXContainerItemProxy */;
		}};'''
            content = content.replace(target_dep_section, target_dep_section + target_dep)

    # 8. Add PBXNativeTarget
    native_target_section = "/* Begin PBXNativeTarget section */"

    deps_block = ""
    if main_target_uuid:
        deps_block = f'''
				{uuids['target_dependency']} /* PBXTargetDependency */,'''

    native_target = f'''
		{uuids['native_target']} /* DashTerm2UITests */ = {{
			isa = PBXNativeTarget;
			buildConfigurationList = {uuids['build_config_list']} /* Build configuration list for PBXNativeTarget "DashTerm2UITests" */;
			buildPhases = (
				{uuids['sources_build_phase']} /* Sources */,
				{uuids['frameworks_build_phase']} /* Frameworks */,
				{uuids['resources_build_phase']} /* Resources */,
			);
			buildRules = (
			);
			dependencies = ({deps_block}
			);
			name = DashTerm2UITests;
			productName = DashTerm2UITests;
			productReference = {uuids['product_ref']} /* DashTerm2UITests.xctest */;
			productType = "com.apple.product-type.bundle.ui-testing";
		}};'''

    content = content.replace(native_target_section, native_target_section + native_target)

    # 9. Add target to project targets list
    # Find the project object and add our target to its targets array
    targets_pattern = re.compile(r'(targets = \()([^)]*\))', re.MULTILINE | re.DOTALL)
    targets_match = targets_pattern.search(content)
    if targets_match:
        targets_start = targets_match.group(1)
        targets_rest = targets_match.group(2)
        new_targets = f"{targets_start}\n\t\t\t\t{uuids['native_target']} /* DashTerm2UITests */,{targets_rest}"
        content = targets_pattern.sub(new_targets, content, count=1)

    # 10. Add XCBuildConfiguration entries
    build_config_section = "/* Begin XCBuildConfiguration section */"

    build_configs = f'''
		{uuids['build_config_debug']} /* Debug */ = {{
			isa = XCBuildConfiguration;
			buildSettings = {{
				CODE_SIGN_STYLE = Manual;
				CURRENT_PROJECT_VERSION = 1;
				GENERATE_INFOPLIST_FILE = YES;
				INFOPLIST_FILE = DashTerm2UITests/Info.plist;
				LD_RUNPATH_SEARCH_PATHS = (
					"$(inherited)",
					"@executable_path/../Frameworks",
					"@loader_path/../Frameworks",
				);
				MARKETING_VERSION = 1.0;
				PRODUCT_BUNDLE_IDENTIFIER = com.dashterm2.uitests;
				PRODUCT_NAME = "$(TARGET_NAME)";
				SWIFT_VERSION = 5.0;
				TEST_TARGET_NAME = DashTerm2;
			}};
			name = Debug;
		}};
		{uuids['build_config_release']} /* Release */ = {{
			isa = XCBuildConfiguration;
			buildSettings = {{
				CODE_SIGN_STYLE = Manual;
				CURRENT_PROJECT_VERSION = 1;
				GENERATE_INFOPLIST_FILE = YES;
				INFOPLIST_FILE = DashTerm2UITests/Info.plist;
				LD_RUNPATH_SEARCH_PATHS = (
					"$(inherited)",
					"@executable_path/../Frameworks",
					"@loader_path/../Frameworks",
				);
				MARKETING_VERSION = 1.0;
				PRODUCT_BUNDLE_IDENTIFIER = com.dashterm2.uitests;
				PRODUCT_NAME = "$(TARGET_NAME)";
				SWIFT_VERSION = 5.0;
				TEST_TARGET_NAME = DashTerm2;
			}};
			name = Release;
		}};
		{uuids['build_config_development']} /* Development */ = {{
			isa = XCBuildConfiguration;
			buildSettings = {{
				CODE_SIGN_STYLE = Manual;
				CURRENT_PROJECT_VERSION = 1;
				GENERATE_INFOPLIST_FILE = YES;
				INFOPLIST_FILE = DashTerm2UITests/Info.plist;
				LD_RUNPATH_SEARCH_PATHS = (
					"$(inherited)",
					"@executable_path/../Frameworks",
					"@loader_path/../Frameworks",
				);
				MARKETING_VERSION = 1.0;
				PRODUCT_BUNDLE_IDENTIFIER = com.dashterm2.uitests;
				PRODUCT_NAME = "$(TARGET_NAME)";
				SWIFT_VERSION = 5.0;
				TEST_TARGET_NAME = DashTerm2;
			}};
			name = Development;
		}};
		{uuids['build_config_nightly']} /* Nightly */ = {{
			isa = XCBuildConfiguration;
			buildSettings = {{
				CODE_SIGN_STYLE = Manual;
				CURRENT_PROJECT_VERSION = 1;
				GENERATE_INFOPLIST_FILE = YES;
				INFOPLIST_FILE = DashTerm2UITests/Info.plist;
				LD_RUNPATH_SEARCH_PATHS = (
					"$(inherited)",
					"@executable_path/../Frameworks",
					"@loader_path/../Frameworks",
				);
				MARKETING_VERSION = 1.0;
				PRODUCT_BUNDLE_IDENTIFIER = com.dashterm2.uitests;
				PRODUCT_NAME = "$(TARGET_NAME)";
				SWIFT_VERSION = 5.0;
				TEST_TARGET_NAME = DashTerm2;
			}};
			name = Nightly;
		}};'''

    content = content.replace(build_config_section, build_config_section + build_configs)

    # 11. Add XCConfigurationList
    config_list_section = "/* Begin XCConfigurationList section */"
    config_list = f'''
		{uuids['build_config_list']} /* Build configuration list for PBXNativeTarget "DashTerm2UITests" */ = {{
			isa = XCConfigurationList;
			buildConfigurations = (
				{uuids['build_config_debug']} /* Debug */,
				{uuids['build_config_release']} /* Release */,
				{uuids['build_config_development']} /* Development */,
				{uuids['build_config_nightly']} /* Nightly */,
			);
			defaultConfigurationIsVisible = 0;
			defaultConfigurationName = Release;
		}};'''

    content = content.replace(config_list_section, config_list_section + config_list)

    # 12. Add product to Products group
    products_pattern = re.compile(r'(\w{24} /\* Products \*/ = \{[^}]*children = \()([^)]*)', re.MULTILINE | re.DOTALL)
    products_match = products_pattern.search(content)
    if products_match:
        products_start = products_match.group(1)
        products_children = products_match.group(2)
        new_products = f"{products_start}\n\t\t\t\t{uuids['product_ref']} /* DashTerm2UITests.xctest */,{products_children}"
        content = products_pattern.sub(new_products, content)

    # Write the modified content
    pbxproj_path.write_text(content)
    print(f"Successfully added DashTerm2UITests target to {pbxproj_path}")
    print(f"UUIDs used: {uuids}")

    return True

if __name__ == "__main__":
    project_path = Path(__file__).parent.parent / "DashTerm2.xcodeproj"
    success = add_uitest_target(project_path)
    sys.exit(0 if success else 1)
