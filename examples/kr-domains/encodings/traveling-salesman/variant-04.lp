% Time-windowed multi-depot traveling salesman.
%
% Multi-depot TSP (variant-03) with arrival-time tracking: each salesman departs
% its depot at time 0, accumulates edge traversal times, and every visit must
% fall within its vertex's optional time window. The total traversed cost is
% minimized.
%
% Assumed inputs (supplied by a separate instance file):
%   vertex(V)          "V is a vertex."
%   edge(X,Y,C)        "an edge from X to Y with cost C."
%   edge_time(X,Y,T)   "traversing the edge from X to Y takes T time units (distinct from cost)."
%   salesman(S)        "S is a salesman."
%   depot(S,V)         "salesman S's depot is vertex V."
%   time_window(V,Min,Max)  "vertex V must be visited at a time T with Min <= T <= Max."
%
% A vertex with no time_window/3 fact is unconstrained in time.

#defined vertex/1.
#defined edge/3.
#defined edge_time/3.
#defined salesman/1.
#defined depot/2.
#defined time_window/3.

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

% No vertex is shared by two salesmen.
:- vertex(V), #count{ S : visited_by(V,S) } > 1.

% returns_to_depot(S): salesman S returns to its depot.
returns_to_depot(S) :- depot(S,V), visited_by(V,S).

% Every salesman must return to its depot.
:- salesman(S), not returns_to_depot(S).

% horizon(M): an upper bound on any feasible tour time. Each salesman uses every
% edge at most once, so the sum of all edge_time/3 values bounds any cumulative
% time. The bound keeps the visit_time/3 recursion's grounding finite: without
% it, the recursive T1+ET head generates a non-terminating closure on any graph
% with a cycle, since gringo over-approximates the traversed_by/4 choice atom
% during grounding.
horizon(M) :- M = #sum{ ET,Src,Dst : edge_time(Src,Dst,ET) }.

% visit_time(V,S,T): salesman S arrives at vertex V at time T. A salesman departs
% its depot at time 0 (implicit) and accumulates edge_time on each arc; the depot
% acquires a visit_time only on the closing arc, so it records the full tour
% duration (and a window on a depot constrains the tour close, not the start).
% The `not depot(S,Src)` guard stops the chain at the close: without it, the
% recursion would re-fire from the depot's outgoing arc once the cycle closes,
% producing spurious repeated arrivals at every vertex.
visit_time(Dst,S,ET) :-
    depot(S,Src),
    traversed_by(Src,Dst,_,S),
    edge_time(Src,Dst,ET).

visit_time(Dst,S,T1+ET) :-
    vertex(Src), visit_time(Src,S,T1),
    traversed_by(Src,Dst,_,S),
    edge_time(Src,Dst,ET),
    not depot(S,Src),
    horizon(M), T1+ET <= M.

% Every visited vertex must have a derived visit_time (catching tour structures
% with no valid arrival time).
:- visited_by(V,S), not visit_time(V,S,_).

% Each visit must fall within the vertex's time window, if one is given.
:- visit_time(V,_,T), time_window(V,Min,_),  T < Min.
:- visit_time(V,_,T), time_window(V,_,Max),  T > Max.

% tour_time(S,T): S's full tour takes T time units (its arrival back at the
% depot). Computed but not constrained here; the surveillance variant (v05) uses
% it.
tour_time(S,T) :- depot(S,V), visit_time(V,S,T).

% Solver directives

% Minimize the total traversed cost.
#minimize{ C,Src,Dst,S : traversed_by(Src,Dst,C,S) }.

#show traversed_by/4.
#show depot/2.
#show visit_time/3.
#show tour_time/2.
