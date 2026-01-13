//
//  iTermSessionPicker.h
//  DashTerm2
//
//  Created by George Nachman on 1/5/19.
//

#import <Cocoa/Cocoa.h>
#import "SplitSelectionView.h"

NS_ASSUME_NONNULL_BEGIN

@class PTYSession;

@interface iTermSessionPicker : NSObject
// True: select, false: insepct
@property (nonatomic) BOOL selectionMode;

- (nullable PTYSession *)pickSession;

@end

NS_ASSUME_NONNULL_END
