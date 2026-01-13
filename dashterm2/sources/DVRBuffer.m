// -*- mode:objc -*-
/*
 **  DVRBuffer.m
 **
 **  Copyright 20101
 **
 **  Author: George Nachman
 **
 **  Project: DashTerm2
 **
 **  Description: Implements a circular in-memory buffer for storing
 **    screen images plus some metadata associated with each frame.
 **
 **  This program is free software; you can redistribute it and/or modify
 **  it under the terms of the GNU General Public License as published by
 **  the Free Software Foundation; either version 2 of the License, or
 **  (at your option) any later version.
 **
 **  This program is distributed in the hope that it will be useful,
 **  but WITHOUT ANY WARRANTY; without even the implied warranty of
 **  MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
 **  GNU General Public License for more details.
 **
 **  You should have received a copy of the GNU General Public License
 **  along with this program; if not, write to the Free Software
 **  Foundation, Inc., 675 Mass Ave, Cambridge, MA 02139, USA.
 */

#import "DVRBuffer.h"

#import "DebugLogging.h"
#import "iTermMalloc.h"
#import "NSArray+iTerm.h"
#import "NSDictionary+iTerm.h"

@implementation DVRBuffer {
    // Points to start of large circular buffer.
    char *store_;

    // Points into store_ after -[reserve:] is called.
    char *scratch_;

    // Total size of storage in bytes.
    long long capacity_;

    // Maps a frame key number to DVRIndexEntry*.
    NSMutableDictionary *index_;

    // First key in index.
    long long firstKey_;

    // Next key number to add to index.
    long long nextKey_;

    // begin may be before or after end. If "-" is an allocated byte and "." is
    // a free byte then you can have one of two cases:
    //
    // begin------end.....
    // ----end....begin---

    // Beginning of circular buffer's used region.
    long long begin_;

    // Non-inclusive end of circular buffer's used regino.
    long long end_;

    // RC-004: Generation counter incremented on structural changes.
    NSUInteger _structuralGeneration;

    // RC-004: Lock to protect concurrent access to buffer and index.
    // Uses NSRecursiveLock because some methods call other methods that also need the lock.
    NSRecursiveLock *_lock;
}

- (instancetype)initWithBufferCapacity:(long long)maxsize {
    self = [super init];
    if (self) {
        capacity_ = maxsize;
        store_ = iTermMalloc(maxsize);
        index_ = [[NSMutableDictionary alloc] initWithCapacity:256];  // DVR frame index
        firstKey_ = 0;
        nextKey_ = 0;
        begin_ = 0;
        end_ = 0;
        // RC-004: Initialize lock for thread-safe access
        _lock = [[NSRecursiveLock alloc] init];
        _lock.name = @"DVRBuffer._lock";
    }
    return self;
}

- (void)dealloc {
    // RC-004 FIX: Hold lock while cleaning up to prevent concurrent access to index_
    // during deallocation. Other threads waiting on the lock will see index_=nil
    // after they acquire it and return safely.
    [_lock lock];
    NSMutableDictionary *indexToRelease = index_;
    index_ = nil; // Set to nil BEFORE releasing so concurrent accessors see nil
    [_lock unlock];

    [indexToRelease release];
    [_lock release];
    free(store_);
    [super dealloc];
}

- (NSDictionary *)exportedIndex {
    [_lock lock];
    // RC-004 FIX: Check index_ is still valid (could be nil during dealloc)
    if (!index_) {
        [_lock unlock];
        return @{};
    }
    // Pre-size based on index count
    NSMutableDictionary *dict = [NSMutableDictionary dictionaryWithCapacity:index_.count];
    for (NSNumber *key in index_) {
        DVRIndexEntry *entry = index_[key];
        if (entry) {
            dict[key] = entry.dictionaryValue;
        }
    }
    [_lock unlock];
    return dict;
}

- (NSDictionary *)dictionaryValue {
    [_lock lock];
    NSDictionary *dict = @{
        @"store" : [NSData dataWithBytes:store_ length:capacity_],
        @"index" : [self exportedIndex],
        @"firstKey" : @(firstKey_),
        @"nextKey" : @(nextKey_),
        @"begin" : @(begin_),
        @"end" : @(end_)
    };
    [_lock unlock];
    return [dict dictionaryByRemovingNullValues];
}

- (BOOL)loadFromDictionary:(NSDictionary *)dict version:(int)version {
    [_lock lock];
    NSData *store = dict[@"store"];
    if (store.length != capacity_) {
        [_lock unlock];
        return NO;
    }
    memmove(store_, store.bytes, store.length);
    if (version != 3) {
        _migrateFromVersion = version;
    }
    scratch_ = 0;

    NSDictionary *indexDict = dict[@"index"];
    for (NSNumber *key in indexDict) {
        NSDictionary *value = indexDict[key];
        DVRIndexEntry *entry = [DVRIndexEntry entryFromDictionaryValue:value];
        if (!entry) {
            [_lock unlock];
            return NO;
        }
        index_[key] = entry;
    }

    firstKey_ = [dict[@"firstKey"] longLongValue];
    nextKey_ = [dict[@"nextKey"] longLongValue];
    begin_ = [dict[@"begin"] longLongValue];
    if (begin_ >= store.length || begin_ < 0) {
        begin_ = 0;
    }
    end_ = [dict[@"end"] longLongValue];
    if (end_ >= store.length || end_ < 0) {
        end_ = 0;
    }
    [_lock unlock];
    return YES;
}

- (BOOL)reserve:(long long)length {
    [_lock lock];
    BOOL hadToFree = NO;
    while (![self _hasSpaceAvailable_locked:length]) {
        // BUG-7344: Gracefully handle case where buffer wraps with first frame as diff
        // This can happen in edge cases - return failure instead of crashing
        if (nextKey_ <= firstKey_) {
            [_lock unlock];
            return NO; // Cannot reserve - no blocks to free
        }
        [self _deallocateBlock_locked];
        hadToFree = YES;
    }
    if (begin_ <= end_) {
        if (capacity_ - end_ >= length) {
            scratch_ = store_ + end_;
        } else {
            scratch_ = store_;
        }
    } else {
        scratch_ = store_ + end_;
    }
    [_lock unlock];
    return hadToFree;
}

- (long long)allocateBlock:(long long)length {
    [_lock lock];
    // BUG-f896: Replace assert with guard - return -1 if no space available
    if (![self _hasSpaceAvailable_locked:length]) {
        DLog(@"BUG-f896: DVRBuffer allocateBlock: insufficient space for length %lld", length);
        [_lock unlock];
        return -1;
    }
    DVRIndexEntry *entry = [[DVRIndexEntry alloc] init];
    entry->position = scratch_ - store_;
    end_ = entry->position + length;
    entry->frameLength = length;
    scratch_ = 0;

    long long key = nextKey_++;
    [index_ setObject:entry forKey:[NSNumber numberWithLongLong:key]];
    [entry release];

    // RC-004: Increment generation after adding a new block
    _structuralGeneration++;

    [_lock unlock];
    return key;
}

// RC-004: Internal locked version - must hold _lock before calling
- (void)_deallocateBlock_locked {
    // RC-004: Increment generation BEFORE removing block to signal readers
    _structuralGeneration++;

    long long key = firstKey_++;
    DVRIndexEntry *entry = [self _entryForKey_locked:key];
    if (entry) {
        begin_ = entry->position + entry->frameLength;
    }
    [index_ removeObjectForKey:[NSNumber numberWithLongLong:key]];
}

- (void)deallocateBlock {
    [_lock lock];
    [self _deallocateBlock_locked];
    [_lock unlock];
}

- (NSUInteger)structuralGeneration {
    [_lock lock];
    NSUInteger gen = _structuralGeneration;
    [_lock unlock];
    return gen;
}

- (void *)blockForKey:(long long)key {
    [_lock lock];
    DVRIndexEntry *entry = [self _entryForKey_locked:key];
    if (!entry) {
        [_lock unlock];
        return NULL;
    }
    void *result = store_ + entry->position;
    [_lock unlock];
    return result;
}

// RC-004: Internal locked version - must hold _lock before calling
- (BOOL)_hasSpaceAvailable_locked:(long long)length {
    if (begin_ <= end_) {
        // ---begin*******end-----
        if (capacity_ - end_ > length) {
            return YES;
        } else if (begin_ > length) {
            return YES;
        } else {
            return NO;
        }
    } else {
        // ***end----begin****
        if (begin_ - end_ > length) {
            return YES;
        } else {
            return NO;
        }
    }
}

- (BOOL)hasSpaceAvailable:(long long)length {
    [_lock lock];
    BOOL result = [self _hasSpaceAvailable_locked:length];
    [_lock unlock];
    return result;
}

- (long long)firstKey {
    [_lock lock];
    long long result = firstKey_;
    [_lock unlock];
    return result;
}

- (long long)lastKey {
    [_lock lock];
    long long result = nextKey_ - 1;
    [_lock unlock];
    return result;
}

// RC-004: Internal locked version - must hold _lock before calling
- (DVRIndexEntry *)_entryForKey_locked:(long long)key {
    if (!index_) {
        return nil;
    }
    return [index_ objectForKey:[NSNumber numberWithLongLong:key]];
}

- (DVRIndexEntry *)entryForKey:(long long)key {
    [_lock lock];
    // RC-004 FIX: Check index_ is still valid (could be nil during dealloc)
    if (!index_) {
        [_lock unlock];
        return nil;
    }
    DVRIndexEntry *result = [self _entryForKey_locked:key];
    if (result) {
        // BUG-1264: Retain result before unlocking to prevent use-after-free.
        // Without this, another thread could deallocate the entry after we unlock
        // but before the caller can retain it, causing objc_retain to crash.
        // RC-004: Use explicit retain+autorelease with nil check to ensure
        // we don't try to retain a potentially invalid pointer.
        [result retain];
        [result autorelease];
    }
    [_lock unlock];
    return result;
}

- (DVRIndexEntry *)firstEntryWithTimestampAfter:(long long)timestamp {
    [_lock lock];
    // RC-004 FIX: Check index_ is still valid (could be nil during dealloc)
    if (!index_) {
        [_lock unlock];
        return nil;
    }
    long long key = [self _firstKeyWithTimestampAfter_locked:timestamp];
    if (key < 0) {
        [_lock unlock];
        return nil;
    }
    DVRIndexEntry *result = index_[[NSNumber numberWithLongLong:key]];
    if (result) {
        // BUG-1264: Retain result before unlocking to prevent use-after-free.
        // RC-004: Use explicit retain+autorelease with nil check.
        [result retain];
        [result autorelease];
    }
    [_lock unlock];
    return result;
}

// RC-004: Internal locked version - must hold _lock before calling
- (long long)_firstKeyWithTimestampAfter_locked:(long long)timestamp {
    // RC-004 FIX: Check index_ is still valid (could be nil during dealloc)
    if (!index_) {
        return -1;
    }
    NSArray<NSNumber *> *frameNumbers = [[index_ allKeys] sortedArrayUsingSelector:@selector(compare:)];
    if (frameNumbers.count == 0) {
        return -1;
    }
    NSArray<NSNumber *> *timestamps = [frameNumbers mapWithBlock:^NSNumber *(NSNumber *frameNumber) {
        DVRIndexEntry *entry = index_[frameNumber];
        // RC-004 FIX: Handle case where entry might be nil
        return entry ? @(entry->info.timestamp) : @(LLONG_MAX);
    }];
    NSUInteger frameNumberIndex =
        [timestamps indexOfObject:@(timestamp + 1)
                    inSortedRange:NSMakeRange(0, frameNumbers.count)
                          options:NSBinarySearchingInsertionIndex
                  usingComparator:^NSComparisonResult(NSNumber *_Nonnull timestamp1, NSNumber *_Nonnull timestamp2) {
                      return [timestamp1 compare:timestamp2];
                  }];
    if (frameNumberIndex == NSNotFound || frameNumberIndex == index_.count) {
        return -1;
    }
    return [frameNumbers[frameNumberIndex] longLongValue];
}

- (long long)firstKeyWithTimestampAfter:(long long)timestamp {
    [_lock lock];
    // RC-004 FIX: Check index_ is still valid (could be nil during dealloc)
    if (!index_) {
        [_lock unlock];
        return -1;
    }
    long long result = [self _firstKeyWithTimestampAfter_locked:timestamp];
    [_lock unlock];
    return result;
}

- (char *)scratch {
    [_lock lock];
    char *result = scratch_;
    [_lock unlock];
    return result;
}

- (ptrdiff_t)offsetOfPointer:(char *)pointer {
    [_lock lock];
    if (pointer == NULL) {
        [_lock unlock];
        return -1;
    }
    if (store_ == NULL) {
        [_lock unlock];
        return -2;
    }
    ptrdiff_t result = pointer - store_;
    [_lock unlock];
    return result;
}

- (long long)capacity {
    // capacity_ is immutable after init, but lock for consistency
    return capacity_;
}

- (BOOL)isEmpty {
    [_lock lock];
    BOOL result = [index_ count] == 0;
    [_lock unlock];
    return result;
}

- (NSData *)dataAtOffset:(ptrdiff_t)offset length:(size_t)length {
    [_lock lock];
    if (offset < 0 || offset >= capacity_) {
        [_lock unlock];
        return nil;
    }
    if (length > capacity_) {
        [_lock unlock];
        return nil;
    }
    // BUG-1267: Off-by-one fix - use > instead of >= because offset+length==capacity
    // means the last byte accessed is at index capacity-1, which is valid
    if (offset + length > capacity_) {
        [_lock unlock];
        return nil;
    }
    NSData *result = [NSData dataWithBytes:store_ + offset length:length];
    [_lock unlock];
    return result;
}

@end
