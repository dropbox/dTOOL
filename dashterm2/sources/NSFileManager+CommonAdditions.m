//
//  NSFileManager+CommonAdditions.m
//  DashTerm2
//
//  Created by George Nachman on 2/24/22.
//

#import "NSFileManager+CommonAdditions.h"

#import "DebugLogging.h"
#import "iTermAutoMasterParser.h"

#import <sys/mount.h>

@implementation NSFileManager (CommonAdditions)

- (BOOL)fileIsLocal:(NSString *)filename
additionalNetworkPaths:(NSArray<NSString *> *)additionalNetworkPaths
 allowNetworkMounts:(BOOL)allowNetworkMounts {
    if (allowNetworkMounts) {
        DLog(@"** Skipping network-mount check because the advanced pref is on!!! **");
        return YES;
    }
    if ([self fileHasForbiddenPrefix:filename additionalNetworkPaths:additionalNetworkPaths]) {
        return NO;
    }

    struct statfs buf;
    const int rc = statfs([filename UTF8String], &buf);
    if (rc != 0) {
        // statfs failed - could be file doesn't exist, permission denied, etc.
        // Be conservative: if we can't verify it's local, treat it as possibly remote.
        // ENOENT is expected for non-existent paths - allow those since fileExistsAtPath
        // will properly handle the check. Other errors (EACCES, EIO, etc.) are suspect.
        if (errno == ENOENT) {
            DLog(@"statfs(%@) returned ENOENT - file doesn't exist, allowing", filename);
            return YES;
        }
        DLog(@"statfs(%@) failed with errno=%d, treating as non-local for safety", filename, errno);
        return NO;
    }
    if (buf.f_flags & MNT_LOCAL) {
        return YES;
    }
    return NO;
}

- (BOOL)fileExistsAtPathLocally:(NSString *)filename
         additionalNetworkPaths:(NSArray<NSString *> *)additionalNetworkPaths
             allowNetworkMounts:(BOOL)allowNetworkMounts {
    if (![self fileIsLocal:filename additionalNetworkPaths:additionalNetworkPaths allowNetworkMounts:allowNetworkMounts]) {
        return NO;
    }
    return [self fileExistsAtPath:filename];
}

- (BOOL)fileHasForbiddenPrefix:(NSString *)filename
        additionalNetworkPaths:(NSArray<NSString *> *)additionalNetworkPaths {
    DLog(@"Additional netwnork paths are: %@", additionalNetworkPaths);
    // Augment list of additional paths with nfs automounter mount points.
    NSMutableArray *networkPaths = [additionalNetworkPaths mutableCopy];
    [networkPaths addObjectsFromArray:[[iTermAutoMasterParser sharedInstance] mountpoints]];
    DLog(@"Including automounter paths, ignoring: %@", networkPaths);

    for (NSString *networkPath in networkPaths) {
        if (!networkPath.length) {
            continue;
        }
        NSString *path;
        if (![networkPath hasSuffix:@"/"]) {
            path = [networkPath stringByAppendingString:@"/"];
        } else {
            path = networkPath;
        }
        if ([filename hasPrefix:path]) {
            DLog(@"Filename %@ has prefix of ignored path %@", filename, networkPath);
            return YES;
        }
    }
    return NO;
}


@end
