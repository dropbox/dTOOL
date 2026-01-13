//
//  iTermStatusBarSetupCollectionViewItem.h
//  DashTerm2
//
//  Created by George Nachman on 6/29/18.
//

#import <Cocoa/Cocoa.h>

@interface iTermStatusBarSetupCollectionViewItem : NSCollectionViewItem

@property (nonatomic, copy) NSString *detailText;
@property (nonatomic) BOOL hideDetail;
@property (nonatomic, strong) NSColor *backgroundColor;

- (void)sizeToFit;

@end
