//
//  PSMOverflowPopUpButton.h
//  NetScrape
//
//  Created by John Pannell on 8/4/04.
//  Copyright 2004 Positive Spin Media. All rights reserved.
//

#import <Cocoa/Cocoa.h>

@interface PSMRolloverButton : NSButton

@property (nonatomic) BOOL allowDrags;

// the regular image
- (void)setUsualImage:(NSImage *)newImage;
- (NSImage *)usualImage;

// the rollover image
- (void)setRolloverImage:(NSImage *)newImage;
- (NSImage *)rolloverImage;

@end

// PSMTahoeRolloverButton requires macOS 26 SDK
// TODO: Re-enable when building with macOS 26 SDK
#if 0  // Disabled until macOS 26 SDK available
NS_AVAILABLE_MAC(26)
@interface PSMTahoeRolloverButton: PSMRolloverButton

- (instancetype)initWithSymbolName:(NSString *)name;

@end
#endif  // Disabled until macOS 26 SDK available

