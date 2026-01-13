//
//  iTermDirectedGraph.m
//  DashTerm2SharedARC
//
//  Created by George Nachman on 23/02/19.
//

#import "iTermDirectedGraph.h"
#import "DebugLogging.h"

@interface NSMutableDictionary (Graph)
- (void)addObject:(id)object toMutableSetWithKey:(id)key;
@end

@implementation NSMutableDictionary (Graph)

- (void)addObject:(id)object toMutableSetWithKey:(id)key {
    NSMutableSet *set = self[key];
    if (!set) {
        set = [[NSMutableSet alloc] initWithCapacity:4];
        self[key] = set;
    }
    [set addObject:object];
}

@end

@implementation iTermDirectedGraph {
    NSMutableSet *_vertexes;
    NSMutableDictionary<id, NSMutableSet *> *_edges;
}

- (instancetype)init {
    self = [super init];
    if (self) {
        _vertexes = [NSMutableSet setWithCapacity:16];
        _edges = [[NSMutableDictionary alloc] initWithCapacity:16];
    }
    return self;
}

- (void)addEdgeFrom:(id)fromVertex to:(id)toVertex {
    [_vertexes addObject:fromVertex];
    [_vertexes addObject:toVertex];
    [_edges addObject:toVertex toMutableSetWithKey:fromVertex];
}

@end

@implementation iTermDirectedGraphCycleDetector {
    iTermDirectedGraph *_graph;
    NSMutableSet *_unexploredVertexes;
    NSMutableSet *_currentVertexes;
}

- (instancetype)initWithDirectedGraph:(iTermDirectedGraph *)directedGraph {
    self = [super init];
    if (self) {
        _graph = directedGraph;
    }
    return self;
}

- (BOOL)containsCycle {
    _unexploredVertexes = _graph.vertexes.mutableCopy;
    _currentVertexes = [[NSMutableSet alloc] initWithCapacity:_graph.vertexes.count];

    while (_unexploredVertexes.count) {
        if ([self searchForCycleBeginningAnywhere]) {
            return YES;
        }
        // BUG-f931: Guard against inconsistent state instead of crashing
        if (_currentVertexes.count != 0) {
            ELog(@"iTermDirectedGraph: currentVertexes not empty after search cycle - count=%lu",
                 (unsigned long)_currentVertexes.count);
            [_currentVertexes removeAllObjects];
        }
    }
    return NO;
}

- (BOOL)searchForCycleBeginningAnywhere {
    id start = _unexploredVertexes.anyObject;
    // BUG-f932: Guard against nil start vertex instead of crashing
    if (!start) {
        ELog(@"iTermDirectedGraph: unexploredVertexes returned nil from anyObject");
        return NO;
    }
    return [self searchFrom:start];
}

- (BOOL)searchFrom:(id)current {
    if ([_currentVertexes containsObject:current]) {
        return YES;
    }
    [_currentVertexes addObject:current];
    [_unexploredVertexes removeObject:current];
    for (id child in _graph.edges[current]) {
        if ([self searchFrom:child]) {
            return YES;
        }
    }
    [_currentVertexes removeObject:current];
    return NO;
}

@end
