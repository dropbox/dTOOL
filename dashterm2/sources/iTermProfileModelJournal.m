//
//  iTermProfileModelJournal.m
//  DashTerm2SharedARC
//
//  Created by George Nachman on 1/20/20.
//

#import "iTermProfileModelJournal.h"

#import "ITAddressBookMgr.h"
#import "NSObject+iTerm.h"

@implementation iTermProfileModelJournalParams
@end

@interface BookmarkJournalEntry()
@property(nonatomic, readwrite) JournalAction action;
@property(nonatomic, readwrite) int index;
@property(nullable, nonatomic, readwrite, strong) NSString *guid;
@property(nonatomic, readwrite, strong) id<iTermProfileModelJournalModel>model;
@property(nonatomic, readwrite, strong) NSArray *tags;
@property(nullable, nonatomic, copy) NSString *identifier;
@end

@implementation BookmarkJournalEntry

+ (BookmarkJournalEntry *)journalWithAction:(JournalAction)action
                                   bookmark:(Profile *)bookmark
                                      model:(id<iTermProfileModelJournalModel>)model
                                 identifier:(NSString *)identifier {
    return [self journalWithAction:action
                          bookmark:bookmark
                             model:model
                             index:0
                        identifier:identifier];
}

+ (instancetype)journalWithAction:(JournalAction)action
                         bookmark:(Profile *)profile
                            model:(id<iTermProfileModelJournalModel>)model
                            index:(int)index
                       identifier:(NSString *)identifier {
    BookmarkJournalEntry *entry = [[BookmarkJournalEntry alloc] init];
    entry.action = action;
    entry.guid = [[profile objectForKey:KEY_GUID] copy];
    entry.model = model;
    entry.index = index;  // BUG-1582: Assign the index parameter
    // BUG-f1652: Use NSArray castFrom for type-safe access (KEY_TAGS default is NSNull)
    NSArray *tags = [NSArray castFrom:[profile objectForKey:KEY_TAGS]];
    entry.tags = tags ? [[NSArray alloc] initWithArray:tags] : @[];
    return entry;
}


@end
