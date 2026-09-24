% Multi-salesman traveling salesman (shared start).
%
% Several salesmen jointly visit every vertex, each tour passing through the
% shared start; minimize the total traversed cost. Each non-start vertex is
% visited by exactly one salesman.
%
% Assumed inputs (supplied by a separate instance file):
%   vertex(V)     "V is a vertex."
%   edge(X,Y,C)   "an edge from X to Y with cost C."
%   start(V)      "V is the start vertex."
%   salesman(S)   "S is a salesman."

#defined vertex/1.
#defined edge/3.
#defined start/1.
#defined salesman/1.

% Domain encoding

% traversed_by(Src,Dst,C,S): the edge from Src to Dst (cost C) is traversed by
% salesman S. At most one outgoing and one incoming edge per (vertex, salesman).
0{ traversed_by(Src,Dst,C,S) : edge(Src,Dst,C) }1 :-
    salesman(S),
    vertex(Src).

0{ traversed_by(Src,Dst,C,S) : edge(Src,Dst,C) }1 :-
    salesman(S),
    vertex(Dst).

% visited_by(V,S): V is visited by S (reachable from the start along S's edges).
visited_by(Dst,S) :-
    start(Src),
    traversed_by(Src,Dst,_,S).

visited_by(Dst,S) :-
    vertex(Src),
    visited_by(Src,S),
    traversed_by(Src,Dst,_,S).

% visited(V): V is visited by some salesman.
visited(V) :- vertex(V), visited_by(V,_).

% No salesman may traverse an edge leaving a vertex it does not visit.
:- traversed_by(Src,_,_,S), not visited_by(Src,S).

% Every vertex must be visited.
:- vertex(V), not visited(V).

% No non-start vertex is shared by two salesmen.
:- vertex(V), not start(V), #count{ S : visited_by(V,S) } > 1.

% returns_to_start(S): salesman S returns to the start.
returns_to_start(S) :- start(V), visited_by(V,S).

% Every salesman must return to the start.
:- salesman(S), not returns_to_start(S).

% Solver directives

% Minimize the total traversed cost.
#minimize{ C,Src,Dst,S : traversed_by(Src,Dst,C,S) }.

#show traversed_by/4.
#show start/1.
