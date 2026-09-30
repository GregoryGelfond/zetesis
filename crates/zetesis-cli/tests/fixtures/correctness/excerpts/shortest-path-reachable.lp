% Extracted S0 fixture for zetesis, not a complete correctness example.
% Upstream MIT: Copyright (c) 2026 Gregory Gelfond. See ../README.md and ../LICENSE.
% Snapshot: 38f0660ded448ed268c5a68759ceb0e2840dd497.

% Synthetic scalar facts authored for this fixture.
% Expected: SAT; exactly one complete model.
vertex(a). vertex(b). vertex(c).
start(a). end(c).
included(a,b,1). included(b,c,1).

% Verbatim upstream rule fragment: encodings/shortest-path/variant-01.lp:35-52.
:- included(_,V,_), start(V).
:- included(V,_,_), end(V).

% reachable(V): V is reachable from the start along included edges.
reachable(Src) :-
    start(Src).

reachable(Dst) :-
    reachable(Src),
    included(Src,Dst,_),
    vertex(Src),
    vertex(Dst).

% The end must be reachable from the start.
:- end(Src), not reachable(Src).

% No included edge may leave an unreachable vertex.
:- included(Src,_,_), not reachable(Src).
