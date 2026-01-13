//
//  main.m
//  DashTerm2SandboxedWorker
//
//  Created by Benedek Kozma on 2020. 12. 23..
//

#import <Foundation/Foundation.h>
#import "DashTerm2SandboxedWorker.h"
#include <sandbox.h>

@interface ServiceDelegate : NSObject <NSXPCListenerDelegate>
@end

#pragma mark - Sandbox

typedef struct iTermSandboxProfile iTermSandboxProfile;
typedef struct iTermSandboxParam iTermSandboxParam;

extern iTermSandboxParam *sandbox_create_params(void);
extern iTermSandboxProfile *sandbox_compile_string(char *program,
                                                   iTermSandboxParam *params,
                                                   char **errorOut);
extern void sandbox_free_profile(iTermSandboxProfile *profile);
extern int sandbox_apply_container(iTermSandboxProfile *profile, uint32_t options);

static BOOL BeginSandbox(void) {
    iTermSandboxParam *const params = sandbox_create_params();

    if (!params) {
        return NO;
    }

    NSBundle *const bundle = [NSBundle bundleForClass:[ServiceDelegate class]];
    NSString *const path = [bundle pathForResource:@"image_decoder" ofType:@"sb"];
    NSString *const profileString = [NSString stringWithContentsOfFile:path
                                                              encoding:NSUTF8StringEncoding error:nil];
    if (!profileString) {
        return NO;
    }

    char *temp = strdup(profileString.UTF8String);
    char *error = NULL;
    iTermSandboxProfile *compiled_profile = sandbox_compile_string(temp, params, &error);
    free(temp);

    const int rc = sandbox_apply_container(compiled_profile, 0);

    if (rc) {
        return NO;
    }

    sandbox_free_profile(compiled_profile);
    return YES;
}

#pragma mark - ServiceDelegate

static BOOL sandboxSuccessful;

@implementation ServiceDelegate

- (BOOL)listener:(NSXPCListener *)listener shouldAcceptNewConnection:(NSXPCConnection *)newConnection {
    if (!sandboxSuccessful) {
        return NO;
    }
    
    newConnection.exportedInterface = [NSXPCInterface interfaceWithProtocol:@protocol(DashTerm2SandboxedWorkerProtocol)];
    DashTerm2SandboxedWorker *exportedObject = [DashTerm2SandboxedWorker new];
    newConnection.exportedObject = exportedObject;
    [newConnection resume];
    
    return YES;
}

@end

int main(int argc, const char *argv[]) {
    sandboxSuccessful = BeginSandbox();

    // BUG-9497: Exit immediately if sandbox profile fails to load.
    // Running without proper sandboxing is a security risk.
    if (!sandboxSuccessful) {
        NSLog(@"Failed to initialize sandbox. Exiting for security.");
        return 1;
    }

    static ServiceDelegate *delegate;
    delegate = [[ServiceDelegate alloc] init];
    NSXPCListener *listener = [NSXPCListener serviceListener];
    listener.delegate = delegate;
    [listener resume];

    return 0;
}
