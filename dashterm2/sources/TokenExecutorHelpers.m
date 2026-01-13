//
//  TokenExecutorHelpers.m
//  DashTerm2
//
//  Created by George Nachman on 2/29/24.
//

#import "TokenExecutorHelpers.h"
#import "VT100Token.h"
#import "VT100TokenPool.h"

VT100Token *CVectorGetVT100Token(const CVector *vector, int index) {
    return (VT100Token *)vector->elements[index];
}
void CVectorSetVT100Token(const CVector *vector, int index, VT100Token *token) {
    [(VT100Token *)vector->elements[index] release];
    vector->elements[index] = [token retain];
}
void CVectorAppendVT100Token(CVector *vector, VT100Token *token) {
    CVectorAppend(vector, (void *)[token retain]);
}

void CVectorRecycleVT100TokensAndDestroy(CVector *vector) {
    VT100TokenPool *pool = [VT100TokenPool sharedPool];
    const int n = CVectorCount(vector);
    for (int i = 0; i < n; i++) {
        VT100Token *token = (VT100Token *)CVectorGet(vector, i);
        // Recycle pooled tokens, release non-pooled ones
        if (token.pooled) {
            [pool recycleToken:token];
        }
        // Always release - balances the retain from CVectorAppend/acquireToken
        [token release];
    }
    CVectorDestroy(vector);
}
