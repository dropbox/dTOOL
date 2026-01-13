//
//  iTermOptionallyBordered.m
//  DashTerm2
//
//  Created by George Nachman on 7/3/21.
//

#import "iTermOptionallyBordered.h"

@implementation NSTextField(OptionallyBordered)
- (void)setOptionalBorderEnabled:(BOOL)enabled {
    self.bordered = YES;
    self.drawsBackground = YES;
}
@end
