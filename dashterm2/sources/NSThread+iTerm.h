//
//  NSThread+iTerm.h
//  DashTerm2
//
//  Created by George Nachman on 4/15/18.
//

#import <Foundation/Foundation.h>

@interface NSThread (iTerm)

// Only DashTerm2 frames
+ (NSArray<NSString *> *)trimCallStackSymbols;

@end
