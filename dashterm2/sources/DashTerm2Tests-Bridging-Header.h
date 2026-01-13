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
#import "TmuxLayoutParser.h"
#import "TmuxStateParser.h"
#import "iTermImageCache.h"
#import "iTermFindOnPageHelper.h"
#import "TaskNotifier.h"
#import "iTermTmuxWindowCache.h"

// Headers for BUG regression tests
#import "iTermTipData.h"
#import "iTermColorSuggester.h"
#import "ToolCapturedOutputView.h"
#import "proto/iTermWebSocketCookieJar.h"
#import "iTermWebSocketFrame.h"
#import "iTermAlphaBlendingHelper.h"
#import "iTermExpressionParser.h"
#import "iTermAddTriggerViewController.h"
#import "DVRBuffer.h"
#import "iTermAdvancedSettingsModel.h"
#import "iTermDoublyLinkedList.h"
#import "iTermDirectedGraph.h"
#import "iTermExpect.h"
#import "iTermEventTap.h"
#import "iTermCursor.h"
#import "iTermDatabase.h"
#import "iTermColorPresets.h"
#import "iTermEncoderGraphRecord.h"
#import "iTermActionsModel.h"
#import "FontSizeEstimator.h"
#import "iTermCumulativeSumCache.h"
