% Traveling salesman (single tour).
%
% Find a minimum-cost cycle through every vertex: exactly one outgoing and one
% incoming edge per vertex, all reachable from the start (which rules out
% subtours).
%
% Assumed inputs (supplied by a separate instance file):
%   vertex(V)     "V is a vertex."
%   edge(X,Y,C)   "an edge from X to Y with cost C."
%   start(V)      "V is the start vertex."

#defined vertex/1.
#defined edge/3.
#defined start/1.

% Domain encoding

% included(X,Y,C): the edge from X to Y (cost C) is on the tour.
% Exactly one outgoing and one incoming tour edge per vertex.
1{ included(Src,Dst,C) : edge(Src,Dst,C) }1 :- vertex(Src).
1{ included(Src,Dst,C) : edge(Src,Dst,C) }1 :- vertex(Dst).

% reachable(V): V is reachable from the start along included edges.
reachable(Src) :-
    start(Src).

reachable(Dst) :-
    reachable(Src), included(Src,Dst,_),
    vertex(Src), vertex(Dst).

% Every vertex must be reachable from the start, ruling out disjoint subtours.
:- vertex(V), not reachable(V).

% cost(X): the total cost of the included edges.
cost(X) :- X = #sum{ C,Src,Dst : included(Src,Dst,C) }.

% Solver directives

% Minimize the tour cost.
#minimize{ C : cost(C) }.

#show included/3.
#show start/1.
