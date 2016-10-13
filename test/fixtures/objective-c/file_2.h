/**
 * Copyright (c) 2016-present, Facebook, Inc.
 * All rights reserved.
 *
 * This source code is licensed under the BSD-style license found in the
 * LICENSE file in the root directory of this source tree. An additional grant
 * of patent rights can be found in the PATENTS file in the same directory.
 */

#import <IGListKit/IGListAdapter.h>
#import <IGListKit/IGListCollectionContext.h>

#import "IGListAdapterProxy.h"
#import "IGListDisplayHandler.h"
#import "IGListSectionMap.h"
#import "IGListWorkingRangeHandler.h"

NS_ASSUME_NONNULL_BEGIN

/// Generate a string representation of a reusable view class when registering with a UICollectionView.
NS_INLINE NSString *IGListReusableViewIdentifier(Class viewClass, NSString * _Nullable kind) {
  return [NSString stringWithFormat:@"%@%@", kind ?: @"", NSStringFromClass(viewClass)];
}

@interface IGListAdapter ()
<
UICollectionViewDataSource,
UICollectionViewDelegateFlowLayout,
IGListCollectionContext
>
{
    __weak UICollectionView *_collectionView;
}



NS_ASSUME_NONNULL_END
