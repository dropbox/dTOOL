//
//  TmuxWindowsTable.m
//  iTerm
//
//  Created by George Nachman on 12/25/11.
//  Copyright (c) 2011 Georgetech. All rights reserved.
//

#import "TmuxWindowsTable.h"
#import "DebugLogging.h"
#import "FutureMethods.h"
#import "NSTextField+iTerm.h"

NSString *kWindowPasteboardType = @"com.dashterm.dashterm2.kWindowPasteboardType";

@implementation TmuxWindowsTable {
    NSMutableArray *model_;
    NSMutableArray *filteredModel_;

    IBOutlet NSTableView *tableView_;
    IBOutlet NSButton *addWindowButton_;
    IBOutlet NSButton *removeWindowButton_;
    IBOutlet NSButton *openInTabsButton_;
    IBOutlet NSButton *openInWindowsButton_;
    IBOutlet NSButton *hideWindowButton_;
    IBOutlet NSSearchField *searchField_;
}

@synthesize delegate = delegate_;

- (instancetype)init {
    self = [super init];
    if (self) {
        model_ = [[NSMutableArray alloc] initWithCapacity:16];  // Typical tmux windows count
    }
    return self;
}

- (void)awakeFromNib {
    [tableView_ setDraggingSourceOperationMask:NSDragOperationLink forLocal:NO];
    [tableView_ setTarget:self];
    [tableView_ setDoubleAction:@selector(didDoubleClickTableView:)];
}

- (void)dealloc {
    [model_ release];
    [filteredModel_ release];
    [super dealloc];
}

- (void)setDelegate:(id<TmuxWindowsTableProtocol>)delegate {
    delegate_ = delegate;
    [delegate_ reloadWindows];
    [self updateEnabledStateOfButtons];
}

- (void)setWindows:(NSArray *)windows {
    [model_ removeAllObjects];
    [model_ addObjectsFromArray:windows];
    [self resetFilteredModel];
    [tableView_ reloadData];
    [self updateEnabledStateOfButtons];
}

- (void)setNameOfWindowWithId:(int)wid to:(NSString *)newName {
    for (int i = 0; i < model_.count; i++) {
        // BUG-f1051: Validate tuple has at least 2 elements before accessing indices
        NSArray *tuple = [model_ objectAtIndex:i];
        if (tuple.count < 2) {
            continue;
        }
        if ([[tuple objectAtIndex:1] intValue] == wid) {
            NSMutableArray *mutableTuple = [model_ objectAtIndex:i];
            [mutableTuple replaceObjectAtIndex:0 withObject:newName];
            break;
        }
    }
    [self resetFilteredModel];
    [tableView_ reloadData];
}

- (NSArray<NSString *> *)names {
    NSMutableArray *names = [NSMutableArray arrayWithCapacity:model_.count];
    for (NSArray *tuple in model_) {
        // BUG-f1052: Validate tuple has at least 1 element before accessing index 0
        if (tuple.count >= 1) {
            [names addObject:[tuple objectAtIndex:0]];
        }
    }
    return names;
}

- (void)updateEnabledStateOfButtons {
    [addWindowButton_ setEnabled:[delegate_ haveSelectedSession] && [self filteredModel].count > 0];
    [removeWindowButton_ setEnabled:[delegate_ haveSelectedSession] && [tableView_ numberOfSelectedRows] > 0];
    [openInTabsButton_ setEnabled:[delegate_ currentSessionSelected] && [tableView_ numberOfSelectedRows] > 1 &&
                                  ![self anySelectedWindowIsOpen]];
    [openInWindowsButton_ setEnabled:[delegate_ currentSessionSelected] && [tableView_ numberOfSelectedRows] > 0 &&
                                     ![self anySelectedWindowIsOpen]];
    if ([openInWindowsButton_ isEnabled] && [tableView_ numberOfSelectedRows] == 1) {
        [openInWindowsButton_ setTitle:@"Open in Window"];
    } else {
        [openInWindowsButton_ setTitle:@"Open in Windows"];
    }
    [hideWindowButton_ setEnabled:[tableView_ numberOfSelectedRows] > 0 && [self allSelectedWindowsAreOpen]];
}

- (void)reloadData {
    [tableView_ reloadData];
}

#pragma mark Interface Builder actions

- (IBAction)addWindow:(id)sender {
    [delegate_ addWindow];
}

- (IBAction)removeWindow:(id)sender {
    // BUG-1079: selectedWindowIds returns NSString objects, not NSNumber
    for (NSString *widString in [self selectedWindowIds]) {
        [delegate_ unlinkWindowWithId:[widString intValue]];
    }
}

- (IBAction)showInWindows:(id)sender {
    [delegate_ showWindowsWithIds:[self selectedWindowIdsAsStrings] inTabs:NO];
    [tableView_ reloadData];
}

- (IBAction)showInTabs:(id)sender {
    [delegate_ showWindowsWithIds:[self selectedWindowIdsAsStrings] inTabs:YES];
    [tableView_ reloadData];
}

- (IBAction)hideWindow:(id)sender {
    // BUG-1079: selectedWindowIds returns NSString objects, not NSNumber
    for (NSString *widString in [self selectedWindowIds]) {
        [delegate_ hideWindowWithId:[widString intValue]];
    }
    [tableView_ reloadData];
}

#pragma mark NSTableViewDataSource

- (NSInteger)numberOfRowsInTableView:(NSTableView *)aTableView {
    return [self filteredModel].count;
}

- (NSView *)tableView:(NSTableView *)tableView viewForTableColumn:(NSTableColumn *)tableColumn row:(NSInteger)row {
    // BUG-1081: Bounds check must come BEFORE accessing the array
    NSArray *model = [self filteredModel];
    if (row < 0 || row >= (NSInteger)model.count) {
        return nil;
    }
    NSArray *values = [model objectAtIndex:row];
    // BUG-f1053: Validate values tuple has at least 2 elements before accessing indices
    if (values.count < 2) {
        return nil;
    }
    NSString *windowID = values[1];
    NSString *name = values[0];

    static NSString *const identifier = @"TmuxWindowIdentifier";
    NSTextField *result = [tableView makeViewWithIdentifier:identifier owner:self];
    if (result == nil) {
        result = [NSTextField it_textFieldForTableViewWithIdentifier:identifier];
    }
    result.font = [NSFont systemFontOfSize:[NSFont systemFontSize]];
    result.editable = YES;
    result.target = self;
    result.action = @selector(didEditWindow:);
    result.stringValue = name;
    result.identifier = windowID;
    // windowID already extracted above with bounds check
    if ([delegate_ haveOpenWindowWithId:[windowID intValue]]) {
        result.alphaValue = 1;
    } else {
        result.alphaValue = 0.5;
    }
    return result;
    ;
}

- (void)didEditWindow:(id)sender {
    NSTextField *textField = sender;
    int windowID = [textField.identifier intValue];
    [delegate_ renameWindowWithId:windowID toName:textField.stringValue];
}

- (void)didDoubleClickTableView:(id)sender {
    NSInteger rowIndex = tableView_.clickedRow;
    if (rowIndex < 0) {
        return;
    }
    NSArray *const model = [self filteredModel];
    // BUG-1081: Add bounds check before access
    if (rowIndex >= (NSInteger)model.count) {
        return;
    }
    NSArray *tuple = model[rowIndex];
    // BUG-f1054: Validate tuple has at least 2 elements before accessing index 1
    if (tuple.count < 2) {
        return;
    }
    NSString *const widString = tuple[1];

    if ([delegate_ haveOpenWindowWithId:widString.intValue]) {
        // Reveal
        [delegate_ tmuxWindowsTableDidSelectWindowWithId:widString.intValue];
        return;
    }

    // Open in window
    [delegate_ showWindowsWithIds:@[ widString ] inTabs:NO];
    [tableView_ reloadData];
}

- (void)tableViewSelectionDidChange:(NSNotification *)aNotification {
    [self updateEnabledStateOfButtons];
}

- (id<NSPasteboardWriting>)tableView:(NSTableView *)tableView pasteboardWriterForRow:(NSInteger)row {
    NSPasteboardItem *pbItem = [[NSPasteboardItem alloc] init];
    NSArray *selectedItems = [[self filteredModel] objectsAtIndexes:[NSIndexSet indexSetWithIndex:row]];
    [pbItem setPropertyList:@[ [delegate_ selectedSessionNumber], selectedItems ] forType:kWindowPasteboardType];
    return pbItem;
}

#pragma mark NSSearchField delegate

- (void)controlTextDidChange:(NSNotification *)aNotification {
    if ([aNotification object] == searchField_) {
        [self resetFilteredModel];
        [tableView_ reloadData];
    }
}

#pragma mark - Private

- (NSArray *)selectedWindowIdsAsStrings {
    NSArray *ids = [self selectedWindowIds];
    NSMutableArray *result = [NSMutableArray arrayWithCapacity:ids.count];
    for (NSString *n in ids) {
        [result addObject:n];
    }
    return result;
}

- (NSArray *)selectedWindowIds {
    NSIndexSet *anIndexSet = [tableView_ selectedRowIndexes];
    NSMutableArray *result = [NSMutableArray arrayWithCapacity:anIndexSet.count];
    NSUInteger i = [anIndexSet firstIndex];
    NSArray *model = [self filteredModel]; // BUG-f1046: Cache model to avoid race conditions

    while (i != NSNotFound) {
        // BUG-f1046: Guard against index out of bounds
        if (i < model.count) {
            NSArray *row = [model objectAtIndex:i];
            if (row.count > 1) {
                [result addObject:[row objectAtIndex:1]];
            } else {
                DLog(@"BUG-f1046: Row at index %lu has insufficient columns (count=%lu)", (unsigned long)i,
                     (unsigned long)row.count);
            }
        } else {
            DLog(@"BUG-f1046: Index %lu out of bounds for filteredModel (count=%lu)", (unsigned long)i,
                 (unsigned long)model.count);
        }
        i = [anIndexSet indexGreaterThanIndex:i];
    }

    return result;
}

- (NSArray *)selectedWindowNames {
    NSIndexSet *anIndexSet = [tableView_ selectedRowIndexes];
    NSMutableArray *result = [NSMutableArray arrayWithCapacity:anIndexSet.count];
    NSUInteger i = [anIndexSet firstIndex];
    NSArray *model = [self filteredModel]; // BUG-f1047: Cache model to avoid race conditions

    while (i != NSNotFound) {
        // BUG-f1047: Guard against index out of bounds
        if (i < model.count) {
            NSArray *row = [model objectAtIndex:i];
            if (row.count > 0) {
                [result addObject:[row objectAtIndex:0]];
            } else {
                DLog(@"BUG-f1047: Row at index %lu is empty", (unsigned long)i);
            }
        } else {
            DLog(@"BUG-f1047: Index %lu out of bounds for filteredModel (count=%lu)", (unsigned long)i,
                 (unsigned long)model.count);
        }
        i = [anIndexSet indexGreaterThanIndex:i];
    }

    return result;
}

- (BOOL)allSelectedWindowsAreOpen {
    // BUG-1079: selectedWindowIds returns NSString objects, not NSNumber
    for (NSString *widString in [self selectedWindowIds]) {
        if (![delegate_ haveOpenWindowWithId:[widString intValue]]) {
            return NO;
        }
    }
    return YES;
}

- (BOOL)anySelectedWindowIsOpen {
    // BUG-1079: selectedWindowIds returns NSString objects, not NSNumber
    for (NSString *widString in [self selectedWindowIds]) {
        if ([delegate_ haveOpenWindowWithId:[widString intValue]]) {
            return YES;
        }
    }
    return NO;
}

- (BOOL)nameMatchesFilter:(NSString *)name {
    NSString *needle = [searchField_ stringValue];

    return (!needle.length ||
            [name rangeOfString:needle
                        options:(NSCaseInsensitiveSearch | NSDiacriticInsensitiveSearch | NSWidthInsensitiveSearch)]
                    .location != NSNotFound);
}

- (NSArray *)filteredModel {
    if (!filteredModel_) {
        filteredModel_ = [[NSMutableArray alloc] initWithCapacity:model_.count];
        for (NSArray *tuple in model_) {
            // BUG-f1055: Validate tuple has at least 1 element before accessing index 0
            if (tuple.count >= 1 && [self nameMatchesFilter:[tuple objectAtIndex:0]]) {
                [filteredModel_ addObject:tuple];
            }
        }
    }
    return filteredModel_;
}

- (void)resetFilteredModel {
    [filteredModel_ release];
    filteredModel_ = nil;
}

@end
