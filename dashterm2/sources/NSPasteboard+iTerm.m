//
//  NSPasteboard+iTerm.m
//  DashTerm2
//
//  Created by George Nachman on 12/11/14.
//
//

#import "NSPasteboard+iTerm.h"
#import "DebugLogging.h"
#import "NSStringITerm.h"
#import "iTermPreferences.h"

@implementation NSPasteboard (iTerm)

- (NSArray *)filenamesOnPasteboardWithShellEscaping:(BOOL)escape forPaste:(BOOL)forPaste {
    // Pre-allocate based on number of URLs on pasteboard (typically 1-10)
    NSArray<NSURL *> *urls = [self readObjectsForClasses:@[ [NSURL class] ] options:0];
    NSMutableArray *results = [NSMutableArray arrayWithCapacity:urls.count];
    for (NSURL *url in urls) {
        NSString *filename = url.path;
        NSDictionary *filenamesAttributes = [[NSFileManager defaultManager] attributesOfItemAtPath:filename error:nil];
        if (([filenamesAttributes fileHFSTypeCode] == 'clpt' && [filenamesAttributes fileHFSCreatorCode] == 'MACS') ||
            [[filename pathExtension] isEqualToString:@"textClipping"] == YES) {
            // Ignore text clippings
            continue;
        }

        if (escape) {
            if (forPaste && [iTermPreferences boolForKey:kPreferenceKeyWrapDroppedFilenamesInQuotesWhenPasting]) {
                filename = [filename quotedStringForPaste];
            } else {
                filename = [filename stringWithEscapedShellCharactersIncludingNewlines:YES];
            }
        }
        if (filename) {
            [results addObject:filename];
        }
    }
    return results;
}

- (NSData *)dataForFirstFile {
    NSString *bestType = [self availableTypeFromArray:@[ NSPasteboardTypeFileURL ]];

    if ([bestType isEqualToString:NSPasteboardTypeFileURL]) {
        NSArray<NSURL *> *urls = [self readObjectsForClasses:@[ [NSURL class] ] options:0];
        if (urls.count > 0) {
            NSString *filename = urls.firstObject.path;
            // BUG-1186: Check file size before loading to prevent OOM on large files.
            // Limit to 100MB which is already quite generous for clipboard operations.
            static const unsigned long long kMaxPasteboardFileSize = 100 * 1024 * 1024;
            NSDictionary *attrs = [[NSFileManager defaultManager] attributesOfItemAtPath:filename error:nil];
            if (!attrs) {
                return nil;
            }
            unsigned long long fileSize = [attrs fileSize];
            if (fileSize > kMaxPasteboardFileSize) {
                DLog(@"Refusing to load pasteboard file larger than %llu bytes: %@ (%llu bytes)",
                     kMaxPasteboardFileSize, filename, fileSize);
                return nil;
            }
            return [NSData dataWithContentsOfFile:filename];
        }
    }
    return nil;
}

@end
