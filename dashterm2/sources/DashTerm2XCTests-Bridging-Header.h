//
//  DashTerm2XCTests-Bridging-Header.h
//  DashTerm2
//
//  Created by George Nachman on 12/8/21.
//
//  NOTE: Use the "ModernTests" scheme to run tests, NOT "DashTerm2Tests".
//  The DashTerm2Tests scheme has linker issues with WebExtensionsFramework.
//

#import "DashTerm2SharedARC-Bridging-Header.h"
#import "../DashTerm2Tests/RemotePreferencesTestHelpers.h"
#import "iTermScriptingWindow.h"

// Additional headers for test coverage
#import "VT100Screen.h"
#import "VT100Grid.h"
#import "LineBuffer.h"
#import "iTermFileDescriptorMultiClient.h"
#import "iTermFileDescriptorServer.h"
