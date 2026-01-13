//
//  iTermLSOF.h
//  DashTerm2
//
//  Created by George Nachman on 11/8/16.
//
//

#import <Foundation/Foundation.h>

@class iTermSocketAddress;
@protocol iTermProcessDataSource;

int iTermProcPidInfoWrapper(int pid, int flavor, uint64_t arg,  void *buffer, int buffersize);

@interface iTermLSOF : NSObject

+ (NSArray<NSString *> *)commandLineArgumentsForProcess:(pid_t)pid execName:(NSString **)execName;
+ (NSString *)commandForProcess:(pid_t)pid execName:(NSString **)execName;
// RC-033: Returns nil on system call failure, empty array if no processes found.
+ (nullable NSArray<NSNumber *> *)allPids;
+ (pid_t)ppidForPid:(pid_t)childPid;
+ (NSNumber *)cachedNumberForPid:(pid_t)pid;
+ (NSString *)nameOfProcessWithPid:(pid_t)thePid isForeground:(BOOL *)isForeground;
+ (NSString *)workingDirectoryOfProcess:(pid_t)pid;
+ (void)asyncWorkingDirectoryOfProcess:(pid_t)pid
                                 queue:(dispatch_queue_t)queue
                                 block:(void (^)(NSString *pwd))block;
+ (pid_t)pidOfFirstChildOf:(pid_t)parentPid;
+ (NSDate *)startTimeForProcess:(pid_t)pid;
+ (id<iTermProcessDataSource>)processDataSource;

@end
