//
//  iTermTouchBarButton.h
//  DashTerm2
//
//  Created by George Nachman on 11/23/16.
//
//

#import <Cocoa/Cocoa.h>

@interface iTermTouchBarButton : NSButton
@property (nonatomic, copy) NSDictionary *keyBindingAction;
@end
