//
//  iTermDoublyLinkedList.m
//  DashTerm2
//
//  Created by George Nachman on 11/5/19.
//

#import "iTermDoublyLinkedList.h"
#import "DebugLogging.h"

NS_ASSUME_NONNULL_BEGIN

@implementation iTermDoublyLinkedList

- (void)prepend:(iTermDoublyLinkedListEntry *)object {
    // BUG-f926: Guard against nil object instead of crashing
    if (!object) {
        ELog(@"iTermDoublyLinkedList: prepend called with nil object");
        return;
    }
    // BUG-f927: Guard against object already in a list
    if (object.dll != nil) {
        ELog(@"iTermDoublyLinkedList: prepend called with object already in a list");
        return;
    }
    // BUG-f928: Guard against object with existing links
    if (object.dllNext != nil || object.dllPrevious != nil) {
        ELog(@"iTermDoublyLinkedList: prepend called with object that has existing links");
        return;
    }

    _count++;
    object.dll = self;
    if (!self.first) {
        // When first is nil, last should also be nil
        if (self.last) {
            ELog(@"iTermDoublyLinkedList: inconsistent state - first is nil but last is not");
        }
        _first = object;
        _last = object;
        return;
    }
    // When first exists, last should also exist
    if (!self.last) {
        ELog(@"iTermDoublyLinkedList: inconsistent state - first exists but last is nil");
    }

    _first.dllPrevious = object;
    object.dllNext = _first;
    _first = object;
}

- (void)remove:(iTermDoublyLinkedListEntry *)object {
    // BUG-f929: Guard against nil object instead of crashing
    if (!object) {
        ELog(@"iTermDoublyLinkedList: remove called with nil object");
        return;
    }
    // BUG-f930: Guard against object not in this list
    if (object.dll != self) {
        ELog(@"iTermDoublyLinkedList: remove called with object not in this list");
        return;
    }
    _count--;
    if (self.first == object) {
        _first = object.dllNext;
    }
    if (self.last == object) {
        _last = object.dllPrevious;
    }
    object.dllPrevious.dllNext = object.dllNext;
    object.dllNext.dllPrevious = object.dllPrevious;
    object.dll = nil;
    object.dllNext = nil;
    object.dllPrevious = nil;
}

@end

@implementation iTermDoublyLinkedListEntry

- (instancetype)initWithObject:(id)object {
    self = [super init];
    if (self) {
        _object = object;
    }
    return self;
}

@end

NS_ASSUME_NONNULL_END
