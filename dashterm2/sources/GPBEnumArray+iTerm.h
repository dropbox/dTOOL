//
//  GPBEnumArray+iTerm.h
//  DashTerm2SharedARC
//
//  Created by George Nachman on 8/5/19.
//

#import "GPBArray.h"

NS_ASSUME_NONNULL_BEGIN

@interface GPBEnumArray (iTerm)

- (BOOL)it_contains:(int32_t)value;
@end

NS_ASSUME_NONNULL_END
