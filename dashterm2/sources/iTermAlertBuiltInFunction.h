//
//  iTermAlertBuiltInFunction.h
//  DashTerm2
//
//  Created by George Nachman on 2/28/19.
//

#import <Foundation/Foundation.h>
#import "iTermBuiltInFunctions.h"

NS_ASSUME_NONNULL_BEGIN

@interface iTermAlertBuiltInFunction : NSObject<iTermBuiltInFunction>
@end

@interface iTermGetStringBuiltInFunction : NSObject<iTermBuiltInFunction>
@end

@interface iTermGetPolyModalAlertBuiltInFunction : NSObject<iTermBuiltInFunction>
@end

NS_ASSUME_NONNULL_END
