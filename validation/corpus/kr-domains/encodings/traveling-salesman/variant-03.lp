% Multi-depot traveling salesman.
%
% Each salesman starts and ends at its own depot (depots are distinct), the
% salesmen jointly visit every vertex exactly once, and the total traversed cost
% is minimized.
%
% Assumed inputs (supplied by a separate instance file):
%   vertex(V)     "V is a vertex."
%   edge(X,Y,C)   "an edge from X to Y with cost C."
%   salesman(S)   "S is a salesman."
%   depot(S,V)    "salesman S's depot is vertex V."

#defined vertex/1.
#defined edge/3.
#defined salesman/1.
#defined depot/2.

% Domain encoding

% traversed_by(Src,Dst,C,S): the edge from Src to Dst (cost C) is traversed by
% salesman S. At most one outgoing and one incoming edge per (vertex, salesman).
0{ traversed_by(Src,Dst,C,S) : edge(Src,Dst,C) }1 :-
    salesman(S),
    vertex(Src).

0{ traversed_by(Src,Dst,C,S) : edge(Src,Dst,C) }1 :-
    salesman(S),
    vertex(Dst).

% visited_by(V,S): V is visited by S (reachable from S's depot).
visited_by(Dst,S) :-
    depot(S,Src),
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

% No vertex is shared by two salesmen. Depots are naturally exclusive to their
% own salesman, so no depot carve-out is needed here.
:- vertex(V), #count{ S : visited_by(V,S) } > 1.

% returns_to_depot(S): salesman S returns to its depot.
returns_to_depot(S) :- depot(S,V), visited_by(V,S).

% Every salesman must return to its depot.
:- salesman(S), not returns_to_depot(S).

% Solver directives

% Minimize the total traversed cost.
#minimize{ C,Src,Dst,S : traversed_by(Src,Dst,C,S) }.

#show traversed_by/4.
#show depot/2.
