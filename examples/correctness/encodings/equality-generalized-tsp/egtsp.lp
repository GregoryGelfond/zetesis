% Equality generalized TSP (E-GTSP, the "one-in-a-set" traveling salesman problem).
%
% A single salesman departs the start vertex, visits exactly one vertex from each
% of a disjoint collection of subsets, and returns to the start, minimizing the
% total tour cost. The start is not a member of any subset.
%
% Assumed inputs (supplied by a separate instance file):
%   vertex(V)       "V is a vertex."
%   edge(X,Y,C)     "an edge from X to Y with cost C."
%   start(V)        "V is the start vertex."
%   in_subset(V,S)  "vertex V belongs to subset S."

#defined vertex/1.
#defined edge/3.
#defined start/1.
#defined in_subset/2.

% Domain encoding

% subset(S): S is a subset the salesman must take exactly one vertex from.
subset(S) :- in_subset(_,S).

% visited(V): V is on the tour. Exactly one vertex per subset, plus the start.
1 { visited(V) : in_subset(V,S) } 1 :- subset(S).

visited(V) :- start(V).

% The start must not be a member of any subset.
:- start(V), in_subset(V,_).

% included(Src,Dst,C): the edge from Src to Dst (cost C) is on the tour. Each
% visited vertex has exactly one outgoing and one incoming tour edge, both to
% other visited vertices.
1 { included(Src,Dst,C) : edge(Src,Dst,C), visited(Dst) } 1 :- visited(Src).
1 { included(Src,Dst,C) : edge(Src,Dst,C), visited(Src) } 1 :- visited(Dst).

% reachable(V): V is reachable from the start along included edges.
reachable(V) :- start(V).
reachable(V) :- reachable(U), included(U,V,_).

% Every visited vertex must be reachable, forcing the included edges into a single
% start-rooted cycle (a subtour disjoint from the start would be unreachable).
:- visited(V), not reachable(V).

% Solver directives

% Minimize the total tour cost.
#minimize { C,Src,Dst : included(Src,Dst,C) }.

#show visited/1.
#show included/3.
#show start/1.
