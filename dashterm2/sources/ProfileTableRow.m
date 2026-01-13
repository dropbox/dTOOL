//
//  ProfileTableRow.m
//  iTerm
//
//  Created by George Nachman on 1/9/12.
//

#import "ProfileTableRow.h"
#import "DebugLogging.h"
#import "ITAddressBookMgr.h"
#import "iTermProfilePreferences.h"

@implementation ProfileTableRow {
    NSString* guid;
    ProfileModel* underlyingModel;
}

- (instancetype)initWithBookmark:(Profile*)bookmark
                 underlyingModel:(ProfileModel*)newUnderlyingModel {
    self = [super init];
    if (self) {
        guid = [[bookmark objectForKey:KEY_GUID] retain];
        self->underlyingModel = [newUnderlyingModel retain];
    }
    return self;
}

- (void)dealloc
{
    [underlyingModel release];
    [guid release];
    [super dealloc];
}

- (Profile*)bookmark
{
    // BUG-7301: Check for stale bookmark and log warning
    Profile *result = [underlyingModel bookmarkWithGuid:guid];
    if (!result) {
        DLog(@"ProfileTableRow: bookmark with guid %@ not found (profile may have been deleted)", guid);
    }
    return result;
}

@end

@implementation ProfileTableRow (KeyValueCoding)

- (NSNumber*)default
{
    BOOL isDefault = [[[self bookmark] objectForKey:KEY_GUID] isEqualToString:[[[ProfileModel sharedInstance] defaultBookmark] objectForKey:KEY_GUID]];
    return [NSNumber numberWithInt:isDefault ? IsDefault : IsNotDefault];
}

- (NSString*)name
{
    return [[self bookmark] objectForKey:KEY_NAME];
}

- (NSString*)shortcut
{
    // BUG-f1652: Use iTermProfilePreferences for type-safe access (KEY_SHORTCUT default is NSNull)
    return [iTermProfilePreferences stringForKey:KEY_SHORTCUT inProfile:[self bookmark]];
}

- (NSString*)command
{
    return [[self bookmark] objectForKey:KEY_COMMAND_LINE];
}

- (NSString*)guid
{
    return [[self bookmark] objectForKey:KEY_GUID];
}

@end

