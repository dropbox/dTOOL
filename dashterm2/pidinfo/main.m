//
//  main.m
//  pidinfo
//
//  Created by George Nachman on 1/11/20.
//

#import <Foundation/Foundation.h>
#import <Security/Security.h>
#import "PIDInfoGitState.h"
#import "pidinfo.h"
#include <stdlib.h>

// Verifies the calling process is signed by the same team as this XPC service.
static BOOL VerifyCallerCodeSignature(NSXPCConnection *connection) {
    pid_t callerPID = connection.processIdentifier;
    if (callerPID <= 0) {
        return NO;
    }

    // Get the code object for the calling process
    SecCodeRef callerCode = NULL;
    NSDictionary *attributes = @{(__bridge NSString *)kSecGuestAttributePid: @(callerPID)};
    OSStatus status = SecCodeCopyGuestWithAttributes(NULL, (__bridge CFDictionaryRef)attributes,
                                                      kSecCSDefaultFlags, &callerCode);
    if (status != errSecSuccess || callerCode == NULL) {
        return NO;
    }

    // Verify the caller's code signature is valid
    status = SecCodeCheckValidity(callerCode, kSecCSDefaultFlags, NULL);
    if (status != errSecSuccess) {
        CFRelease(callerCode);
        return NO;
    }

    // Get signing info to verify team identifier
    CFDictionaryRef signingInfo = NULL;
    status = SecCodeCopySigningInformation(callerCode, kSecCSSigningInformation, &signingInfo);
    CFRelease(callerCode);

    if (status != errSecSuccess || signingInfo == NULL) {
        return NO;
    }

    // Extract team identifier from caller
    NSString *callerTeamID = ((__bridge NSDictionary *)signingInfo)[(__bridge NSString *)kSecCodeInfoTeamIdentifier];
    CFRelease(signingInfo);

    if (!callerTeamID) {
        // For development builds, allow connections from unsigned processes if we're also unsigned
        SecCodeRef selfCode = NULL;
        if (SecCodeCopySelf(kSecCSDefaultFlags, &selfCode) == errSecSuccess) {
            CFDictionaryRef selfInfo = NULL;
            if (SecCodeCopySigningInformation(selfCode, kSecCSSigningInformation, &selfInfo) == errSecSuccess) {
                NSString *selfTeamID = ((__bridge NSDictionary *)selfInfo)[(__bridge NSString *)kSecCodeInfoTeamIdentifier];
                CFRelease(selfInfo);
                CFRelease(selfCode);
                // Both unsigned - allow for development
                return selfTeamID == nil;
            }
            CFRelease(selfCode);
        }
        return NO;
    }

    // Get our own team identifier
    SecCodeRef selfCode = NULL;
    status = SecCodeCopySelf(kSecCSDefaultFlags, &selfCode);
    if (status != errSecSuccess || selfCode == NULL) {
        return NO;
    }

    status = SecCodeCopySigningInformation(selfCode, kSecCSSigningInformation, &signingInfo);
    CFRelease(selfCode);

    if (status != errSecSuccess || signingInfo == NULL) {
        return NO;
    }

    NSString *selfTeamID = ((__bridge NSDictionary *)signingInfo)[(__bridge NSString *)kSecCodeInfoTeamIdentifier];
    CFRelease(signingInfo);

    // Verify team identifiers match
    return [callerTeamID isEqualToString:selfTeamID];
}

@interface ServiceDelegate : NSObject <NSXPCListenerDelegate>
@end

@implementation ServiceDelegate

- (BOOL)listener:(NSXPCListener *)listener shouldAcceptNewConnection:(NSXPCConnection *)newConnection {
    // BUG-9496: Verify caller before accepting XPC connection
    if (!VerifyCallerCodeSignature(newConnection)) {
        [newConnection invalidate];
        return NO;
    }

    // Configure the connection.
    // First, set the interface that the exported object implements.
    newConnection.exportedInterface = [NSXPCInterface interfaceWithProtocol:@protocol(pidinfoProtocol)];

    // Next, set the object that the connection exports. All messages sent on the connection to this service will be sent to the exported object to handle. The connection retains the exported object.
    pidinfo *exportedObject = [pidinfo new];
    newConnection.exportedObject = exportedObject;

    // Resuming the connection allows the system to deliver more incoming messages.
    [newConnection resume];

    // Returning YES from this method tells the system that you have accepted this connection. If you want to reject the connection for some reason, call -invalidate on the connection and return NO.
    return YES;
}

@end

int main(int argc, const char *argv[])
{
    // pidinfo --git-state /path/to/repo <timeout>
    if (argc == 4 && !strcmp(argv[1], "--git-state")) {
        @autoreleasepool {
            PIDInfoGetGitState(argv[2], atoi(argv[3]));
        }
        return 0;
    }
    // Create the delegate for the service.
    ServiceDelegate *delegate = [ServiceDelegate new];
    
    // Set up the one NSXPCListener for this service. It will handle all incoming connections.
    NSXPCListener *listener = [NSXPCListener serviceListener];
    listener.delegate = delegate;
    
    // Resuming the serviceListener starts this service. This method does not return.
    [listener resume];
    return 0;
}
