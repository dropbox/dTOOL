//
//  PTYTextView+MouseHandler.h
//  DashTerm2
//
//  Created by George Nachman on 7/21/25.
//

#import "DashTerm2SharedARC-Swift.h"
#import "iTermSecureKeyboardEntryController.h"
#import "PTYMouseHandler.h"

@interface PTYTextView(MouseHandler)<iTermFocusFollowsMouseDelegate, iTermSecureInputRequesting, PTYMouseHandlerDelegate, iTermFocusFollowsMouseFocusReceiver>
@end

