//
//  DirectoriesPopup.m
//  iTerm
//
//  Created by George Nachman on 5/2/14.
//
//

#import "DirectoriesPopup.h"
#import "iTermRecentDirectoryMO.h"
#import "iTermRecentDirectoryMO+Additions.h"
#import "iTermShellHistoryController.h"
#import "NSDateFormatterExtras.h"
#import "PopupModel.h"

@implementation DirectoriesPopupEntry

- (void)dealloc {
    [_entry release];
    [super dealloc];
}

@end

@implementation DirectoriesPopupWindowController {
    IBOutlet NSTableView *_tableView;
    IBOutlet NSTableColumn *_mainColumn;
}

- (instancetype)init {
    self = [super initWithWindowNibName:@"DirectoriesPopup"
                               tablePtr:nil
                                  model:[[[PopupModel alloc] init] autorelease]];
    if (self) {
        [self window];
        [self setTableView:_tableView];
    }

    return self;
}

- (void)dealloc {
    [_tableView setDelegate:nil];
    [_tableView setDataSource:nil];
    [super dealloc];
}

- (void)loadDirectoriesForHost:(id<VT100RemoteHostReading>)host {
    [[self unfilteredModel] removeAllObjects];
    for (iTermRecentDirectoryMO *entry in [[iTermShellHistoryController sharedInstance] directoriesSortedByScoreOnHost:host]) {
        DirectoriesPopupEntry *popupEntry = [[[DirectoriesPopupEntry alloc] init] autorelease];
        popupEntry.entry = entry;
        [popupEntry setMainValue:popupEntry.entry.path];
        [[self unfilteredModel] addObject:popupEntry];
    }
    [self reloadData:YES];
}

// BUG-386: Add bounds check to prevent array index out of bounds crash
- (id)tableView:(NSTableView *)aTableView
    objectValueForTableColumn:(NSTableColumn *)aTableColumn
            row:(NSInteger)rowIndex {
    NSInteger convertedIndex = [self convertIndex:rowIndex];
    NSArray *model = [self model];
    if (convertedIndex < 0 || convertedIndex >= (NSInteger)model.count) {
        return nil;
    }
    DirectoriesPopupEntry* entry = [model objectAtIndex:convertedIndex];
    if ([[aTableColumn identifier] isEqualToString:@"date"]) {
        // Date
        return [NSDateFormatter dateDifferenceStringFromDate:[NSDate dateWithTimeIntervalSinceReferenceDate:entry.entry.lastUse.doubleValue]];
    } else {
        // Contents
        return [super tableView:aTableView objectValueForTableColumn:aTableColumn row:rowIndex];
    }
}

// BUG-387: Add bounds check to prevent array index out of bounds crash in rowSelected
- (void)rowSelected:(id)sender {
    if ([_tableView selectedRow] >= 0) {
        NSInteger convertedIndex = [self convertIndex:[_tableView selectedRow]];
        NSArray *model = [self model];
        if (convertedIndex < 0 || convertedIndex >= (NSInteger)model.count) {
            return;
        }
        DirectoriesPopupEntry* entry = [model objectAtIndex:convertedIndex];
        [self.delegate popupInsertText:entry.entry.path popup:self];
        [super rowSelected:sender];
    }
}

- (NSAttributedString *)shrunkToFitAttributedString:(NSAttributedString *)attributedString
                                            inEntry:(DirectoriesPopupEntry *)entry
                                     baseAttributes:(NSDictionary *)baseAttributes {
    NSIndexSet *indexes =
        [[iTermShellHistoryController sharedInstance] abbreviationSafeIndexesInRecentDirectory:entry.entry];
    return [entry.entry attributedStringForTableColumn:_mainColumn
                               basedOnAttributedString:attributedString
                                        baseAttributes:baseAttributes
                            abbreviationSafeComponents:indexes];
}

- (NSString *)truncatedMainValueForEntry:(DirectoriesPopupEntry *)entry {
    // Don't allow truncation because directories shouldn't be unreasonably big.
    return entry.entry.path;
}

- (BOOL)shouldEscapeShellCharacters {
    return YES;
}

@end
