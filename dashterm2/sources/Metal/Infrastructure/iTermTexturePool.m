//
//  iTermTexturePool.m
//  DashTerm2
//
//  Created by George Nachman on 12/31/17.
//

#import "iTermTexturePool.h"

#import "iTermPowerManager.h"
#import "NSObject+iTerm.h"
#import <os/lock.h>

static void *iTermTexturePoolAssociatedObjectKeyGeneration = "iTermTexturePoolAssociatedObjectKeyGeneration";

NS_ASSUME_NONNULL_BEGIN

@implementation iTermTexturePool {
    NSMutableArray<id<MTLTexture>> *_textures;
    vector_uint2 _size;
    NSNumber *_generation;
    os_unfair_lock _lock; // Performance optimization: os_unfair_lock has ~10x lower overhead than @synchronized
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _generation = @0;
        // Texture pools typically hold 4-8 textures for reuse; pre-allocate to avoid growth
        _textures = [NSMutableArray arrayWithCapacity:8];
        _lock = OS_UNFAIR_LOCK_INIT;
        [[NSNotificationCenter defaultCenter] addObserver:self selector:@selector(powerManagerMetalAllowedDidChange:) name:iTermPowerManagerMetalAllowedDidChangeNotification object:nil];
    }
    return self;
}

- (void)dealloc {
    [[NSNotificationCenter defaultCenter] removeObserver:self];
}

- (nullable id<MTLTexture>)requestTextureOfSize:(vector_uint2)size {
    os_unfair_lock_lock(&_lock);
    if (size.x != _size.x || size.y != _size.y) {
        _size = size;
        [_textures removeAllObjects];
        os_unfair_lock_unlock(&_lock);
        return nil;
    }
    if (_textures.count) {
        id<MTLTexture> result = _textures.firstObject;
        [_textures removeObjectAtIndex:0];
        [self stampTextureWithGeneration:result];
        os_unfair_lock_unlock(&_lock);
        return result;
    } else {
        os_unfair_lock_unlock(&_lock);
        return nil;
    }
}

- (void)returnTexture:(id<MTLTexture>)texture {
    os_unfair_lock_lock(&_lock);
    if (texture.width == _size.x && texture.height == _size.y) {
        NSNumber *generation = [(NSObject *)texture it_associatedObjectForKey:iTermTexturePoolAssociatedObjectKeyGeneration];
        if ([NSObject object:generation isEqualToObject:_generation]) {
            [_textures addObject:texture];
        }
    }
    os_unfair_lock_unlock(&_lock);
}

- (void)powerManagerMetalAllowedDidChange:(NSNotification *)notification {
    NSNumber *allowedNumber = notification.object;
    if (!allowedNumber.boolValue) {
        os_unfair_lock_lock(&_lock);
        [_textures removeAllObjects];
        _generation = @(_generation.integerValue + 1);
        os_unfair_lock_unlock(&_lock);
    }
}

- (void)stampTextureWithGeneration:(id<MTLTexture>)texture {
    [(NSObject *)texture it_setAssociatedObject:_generation forKey:iTermTexturePoolAssociatedObjectKeyGeneration];
}

@end

@implementation iTermPooledTexture {
    __weak iTermTexturePool *_pool;
}

- (instancetype)initWithTexture:(id<MTLTexture>)texture pool:(iTermTexturePool *)pool {
    self = [super init];
    if (self) {
        _texture = texture;
        _pool = pool;
        [pool stampTextureWithGeneration:texture];
    }
    return self;
}

- (void)dealloc {
    [_pool returnTexture:_texture];
}

@end


NS_ASSUME_NONNULL_END
