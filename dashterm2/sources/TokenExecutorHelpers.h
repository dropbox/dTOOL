//
//  TokenExecutorHelpers.h
//  DashTerm2
//
//  Created by George Nachman on 2/29/24.
//

#import <Foundation/Foundation.h>
#import "CVector.h"

@class VT100Token;

NS_ASSUME_NONNULL_BEGIN

// This keeps Swift from trying to do dynamic casting in a performance-critical loop.
VT100Token *CVectorGetVT100Token(const CVector *vector, int index);
void CVectorSetVT100Token(const CVector *vector, int index, VT100Token *token);
void CVectorAppendVT100Token(CVector *vector, VT100Token *token);

/// Recycles all VT100Token objects in the vector to the pool and destroys the vector.
/// This is more efficient than releasing tokens because it reuses them instead of deallocating.
void CVectorRecycleVT100TokensAndDestroy(CVector *vector);

NS_ASSUME_NONNULL_END
