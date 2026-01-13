//
//  PTYTab+Scripting.m
//  DashTerm2
//
//  Created by George Nachman on 8/26/14.
//
//

#import "PTYTab+Scripting.h"

#import "iTermScriptingWindow.h"
#import "PseudoTerminal.h"
#import "PTYWindow.h"

@implementation PTYTab (Scripting)

- (NSScriptObjectSpecifier *)objectSpecifier {
  id classDescription = [NSClassDescription classDescriptionForClass:[iTermScriptingWindow class]];
  NSInteger index = [[self realParentWindow] indexOfTab:self];
  // BUG-1032: Check for NSNotFound to avoid invalid specifier for detached tabs
  if (index == NSNotFound) {
    return nil;
  }
  return [[[NSIndexSpecifier alloc] initWithContainerClassDescription:classDescription
                                                   containerSpecifier:[self.realParentWindow.window objectSpecifier]
                                                                  key:@"tabs"
                                                                index:index] autorelease];
}

- (id)valueInSessionsAtIndex:(unsigned)anIndex {
  // BUG-1033: Add bounds check to prevent crash
  NSArray *sessions = [self sessions];
  if (anIndex >= sessions.count) {
    return nil;
  }
  return sessions[anIndex];
}

- (id)valueForKey:(NSString *)key {
  if ([key isEqualToString:@"currentSession"]) {
    return [self activeSession];
  } else if ([key isEqualToString:@"isProcessing"]) {
    return @([self isProcessing]);
  } else if ([key isEqualToString:@"icon"]) {
    return [self icon];
  } else if ([key isEqualToString:@"objectCount"]) {
    return @([self objectCount]);
  } else if ([key isEqualToString:@"sessions"]) {
    return [self sessions];
  } else if ([key isEqualToString:@"indexOfTab"]) {
    return [self indexOfTab];
  } else if ([key isEqualToString:@"title"]) {
    return [self title];
  } else {
    return nil;
  }
}

- (id)valueWithUniqueID:(id)uniqueID inPropertyWithKey:(NSString *)key {
    if ([key isEqualToString:@"sessions"]) {
        return [self valueInSessionsWithWithUniqueID:uniqueID];
    }
    return nil;
}

- (id)valueInSessionsWithWithUniqueID:(NSString *)guid {
    for (PTYSession *session in self.sessions) {
        if ([session.guid isEqual:guid]) {
            return session;
        }
    }
    return nil;
}

- (NSUInteger)countOfSessions {
  return [[self sessions] count];
}

- (NSNumber *)indexOfTab {
  return @([self number]);
}

- (void)handleCloseCommand:(NSScriptCommand *)scriptCommand {
    // BUG-1034: Use realParentWindow instead of parentWindow (works during instant replay)
    [[self realParentWindow] closeTab:self];
}

- (void)handleSelectCommand:(NSScriptCommand *)scriptCommand {
    // BUG-1035: Use realParentWindow instead of parentWindow (works during instant replay)
    [[[self realParentWindow] tabView] selectTabViewItemWithIdentifier:self];
}

@end
