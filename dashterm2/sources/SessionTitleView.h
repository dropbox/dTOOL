//
//  SessionTitleView.h
//  iTerm
//
//  Created by George Nachman on 10/21/11.
//  Copyright 2011 George Nachman. All rights reserved.
//

#import <Cocoa/Cocoa.h>
#import "iTermStatusBarViewController.h"
#import "NSView+iTerm.h"

@protocol SessionTitleViewDelegate <NSObject>

- (NSMenu *)menu;
- (void)close;
- (void)beginDrag;
- (void)doubleClickOnTitleView;
- (void)sessionTitleViewBecomeFirstResponder;
- (NSColor *)sessionTitleViewBackgroundColor;

@end

@interface SessionTitleView : NSView<iTermStatusBarContainer, iTermViewScreenNotificationHandling>

@property(nonatomic, copy) NSString *title;
@property(nonatomic, weak) id<SessionTitleViewDelegate> delegate;
@property(nonatomic, assign) double dimmingAmount;
@property(nonatomic, assign) int ordinal;
@property(nonatomic, assign) BOOL showsAILockIndicator;

- (void)updateTextColor;
- (void)updateBackgroundColor;

@end
