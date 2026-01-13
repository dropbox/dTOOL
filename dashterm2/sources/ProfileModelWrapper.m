//
//  ProfileModelWrapper.m
//  iTerm
//
//  Created by George Nachman on 1/9/12.
//

#import "ProfileModelWrapper.h"
#import "DebugLogging.h"
#import "NSArray+iTerm.h"

@implementation ProfileModelWrapper {
    ProfileModel *underlyingModel;
    NSMutableArray<ProfileTableRow *> *bookmarks;
    NSMutableString *filter;
    NSArray *sortDescriptors;
}

- (instancetype)initWithModel:(ProfileModel *)model profileTypes:(ProfileType)profileTypes {
    self = [super init];
    if (self) {
        _profileTypes = profileTypes;
        underlyingModel = model;
        bookmarks = [[NSMutableArray alloc] initWithCapacity:32];  // Filtered bookmarks
        filter = [[NSMutableString alloc] init];
        [self sync];
    }
    return self;
}

- (void)dealloc {
    [bookmarks release];
    [filter release];
    [_lockedGuid release];
    [super dealloc];
}

- (void)setSortDescriptors:(NSArray *)newSortDescriptors {
    [sortDescriptors autorelease];
    sortDescriptors = [newSortDescriptors retain];
}

- (void)dump {
    for (int i = 0; i < [self numberOfBookmarks]; ++i) {
        DLog(@"Dump of %p: At %d: %@", self, i, [[self profileTableRowAtIndex:i] name]);
    }
}

- (void)sort {
    if ([sortDescriptors count] > 0) {
        [bookmarks sortUsingDescriptors:sortDescriptors];
    }
}

- (int)numberOfBookmarks {
    return [bookmarks count];
}

// BUG-3115: Add bounds check to prevent array index out of bounds crash
- (ProfileTableRow *)profileTableRowAtIndex:(int)i {
    if (i < 0 || (NSUInteger)i >= bookmarks.count) {
        return nil;
    }
    return [bookmarks objectAtIndex:i];
}

// BUG-3115: Add bounds check to prevent array index out of bounds crash
- (Profile *)profileAtIndex:(int)i {
    if (i < 0 || (NSUInteger)i >= bookmarks.count) {
        return nil;
    }
    return [[bookmarks objectAtIndex:i] bookmark];
}

- (NSArray<Profile *> *)profiles {
    return [bookmarks mapWithBlock:^id(ProfileTableRow *row) {
        return row.bookmark;
    }];
}

- (int)indexOfProfileWithGuid:(NSString *)guid {
    for (int i = 0; i < [bookmarks count]; ++i) {
        if ([[[bookmarks objectAtIndex:i] guid] isEqualToString:guid]) {
            return i;
        }
    }
    return -1;
}

- (ProfileModel *)underlyingModel {
    return underlyingModel;
}

- (void)sync {
    DLog(@"Synchronize profile model wrapper with underlying bookmarks");
    [bookmarks removeAllObjects];

    NSArray *filteredBookmarks = [underlyingModel profileIndicesMatchingFilter:filter
                                                                        orGuid:self.lockedGuid
                                                                        ofType:_profileTypes];
    for (NSNumber *n in filteredBookmarks) {
        int i = [n intValue];
        [bookmarks addObject:[[[ProfileTableRow alloc] initWithBookmark:[underlyingModel profileAtIndex:i]
                                                        underlyingModel:underlyingModel] autorelease]];
    }
    [self sort];
    DLog(@"There are now %d bookmarks", (int)bookmarks.count);
}

- (void)moveBookmarkWithGuid:(NSString *)guid toIndex:(int)row {
    // Make the change locally.
    int origRow = [self indexOfProfileWithGuid:guid];
    // BUG-397: Bounds check before accessing bookmarks array to prevent crash
    // indexOfProfileWithGuid can return -1 if guid is not found
    if (origRow < 0 || origRow >= (int)bookmarks.count) {
        return;
    }
    if (row < 0 || row > (int)bookmarks.count) {
        return;
    }
    if (origRow < row) {
        [bookmarks insertObject:[bookmarks objectAtIndex:origRow] atIndex:row];
        [bookmarks removeObjectAtIndex:origRow];
    } else if (origRow > row) {
        ProfileTableRow *temp = [[bookmarks objectAtIndex:origRow] retain];
        [bookmarks removeObjectAtIndex:origRow];
        [bookmarks insertObject:temp atIndex:row];
        [temp release];
    }
}

- (void)pushOrderToUnderlyingModel {
    // Since we may have a filter, let's ensure that the visible bookmarks occur
    // in the same order in the underlying model without regard to how invisible
    // bookmarks fit into the order. This also prevents instability when the
    // reload happens.
    int i = 0;
    for (ProfileTableRow *theRow in bookmarks) {
        [underlyingModel moveGuid:[theRow guid] toRow:i++];
    }
    [underlyingModel recordSortOrder];
    [underlyingModel rebuildMenus];
    [underlyingModel flush];
}

- (NSArray *)names {
    // Pre-allocate based on number of bookmarks
    NSMutableArray *array = [NSMutableArray arrayWithCapacity:bookmarks.count];
    for (ProfileTableRow *theRow in bookmarks) {
        [array addObject:[theRow name]];
    }
    return array;
}

- (NSArray *)sortDescriptors {
    return sortDescriptors;
}

- (void)setFilter:(NSString *)newFilter {
    self.lockedGuid = nil;
    [filter release];
    filter = [[NSMutableString stringWithString:newFilter] retain];
}

- (void)setProfileTypes:(ProfileType)profileTypes {
    _profileTypes = profileTypes;
    self.lockedGuid = nil;
}

@end
