//
//  iTermActionsMenuController.m
//  DashTerm2SharedARC
//
//  Created by George Nachman on 9/8/20.
//

#import "iTermActionsMenuController.h"

@implementation iTermActionsMenuController {
    IBOutlet NSMenuItem *_menu;
}

- (void)awakeFromNib {
    [iTermActionsDidChangeNotification subscribe:self selector:@selector(actionsDidChange:)];
    [self reload];
}

- (void)actionsDidChange:(iTermActionsDidChangeNotification *)notification {
    switch (notification.mutationType) {
        case iTermActionsDidChangeMutationTypeEdit:
            [self reloadIndex:notification.index];
            break;
        case iTermActionsDidChangeMutationTypeDeletion:
            [self deleteIndexes:notification.indexSet];
            break;
        case iTermActionsDidChangeMutationTypeInsertion:
            [self insertAtIndex:notification.index];
            break;
        case iTermActionsDidChangeMutationTypeMove:
            [self moveIndexes:notification.indexSet to:notification.index];
            break;
        case iTermActionsDidChangeMutationTypeFullReplacement:
            [self reload];
            break;
    }
}

- (void)reload {
    [_menu.submenu removeAllItems];
    [[[iTermActionsModel sharedInstance] actions] enumerateObjectsUsingBlock:
     ^(iTermAction * _Nonnull action, NSUInteger idx, BOOL * _Nonnull stop) {
        [self add:action];
    }];
}

- (void)add:(iTermAction *)action {
    NSMenuItem *item = [[NSMenuItem alloc] initWithTitle:action.title
                                                  action:@selector(applyAction:)
                                           keyEquivalent:@""];
    item.representedObject = action;
    [_menu.submenu addItem:item];
}

// BUG-383: Add bounds check to prevent array index out of bounds crash
- (void)reloadIndex:(NSInteger)index {
    NSArray *actions = [[iTermActionsModel sharedInstance] actions];
    if (index < 0 || (NSUInteger)index >= actions.count) {
        return;
    }
    if (index >= _menu.submenu.numberOfItems) {
        return;
    }
    iTermAction *action = [actions objectAtIndex:index];
    NSMenuItem *item = [_menu.submenu itemAtIndex:index];
    item.title = action.title;
    item.representedObject = action;
}

- (void)deleteIndexes:(NSIndexSet *)indexes {
    [indexes enumerateIndexesWithOptions:NSEnumerationReverse usingBlock:^(NSUInteger idx, BOOL * _Nonnull stop) {
        [_menu.submenu removeItemAtIndex:idx];
    }];
}

- (void)insertAtIndex:(NSInteger)index {
    [_menu.submenu insertItem:[[NSMenuItem alloc] init] atIndex:index];
    [self reloadIndex:index];
}

- (void)moveIndexes:(NSIndexSet *)sourceIndexes to:(NSInteger)destinationIndex {
    [self deleteIndexes:sourceIndexes];
    for (NSInteger i = 0; i < sourceIndexes.count; i++) {
        [self insertAtIndex:destinationIndex + i];
    }
}

@end
