% Shortest path with ordering constraints.
%
% Minimize the total cost of a start-to-end path subject to before/after
% ordering constraints, with the number of edges as a tie-breaker.
%
% Assumed inputs (supplied by a separate instance file):
%   vertex(V)     "V is a vertex."
%   edge(X,Y,C)   "an edge from X to Y with cost C."
%   start(V)      "V is the start vertex."
%   end(V)        "V is the end vertex."
%   before(X,Y)   "if Y is visited, X must be visited at some point before Y."
%   after(X,Y)    "if Y is visited, X must be visited at some point after Y."

#defined vertex/1.
#defined edge/3.
#defined start/1.
#defined end/1.
#defined before/2.
#defined after/2.

% Domain encoding

% included(X,Y,C): the edge from X to Y (cost C) lies on the candidate path.
{ included(Src,Dst,C) : edge(Src,Dst,C) }.

% Path structure. Together these four make the included edges a simple path: at
% most one included edge leaves a vertex and at most one enters it, nothing
% enters the start, and nothing leaves the end. The ordering constraints below
% depend on this: on a path, reachable_from/2 is the visitation order, whereas
% over an arbitrary edge set it is only reachability.
%
% Only the out-degree bound and the no-edge-out-of-the-end rule are independent.
% The other two follow from them together with the no-dangling-edge constraint
% below: capping out-degree makes the walk from the start deterministic,
% no-dangling puts every included edge on that walk, nothing-leaves-end forces
% the walk to terminate, and a terminating deterministic walk cannot revisit a
% vertex. They are stated anyway, so that the object is read off the program
% rather than derived.
:- vertex(V), #count{ Dst,C : included(V,Dst,C) } > 1.
:- vertex(V), #count{ Src,C : included(Src,V,C) } > 1.
:- included(_,V,_), start(V).
:- included(V,_,_), end(V).

% reachable_from(X,Y): Y is reachable from X along included edges (transitive).
reachable_from(Src,Dst) :-
    included(Src,Dst,_).

reachable_from(Src,Dst) :-
    vertex(Tmp),
    included(Src,Tmp,_),
    reachable_from(Tmp,Dst).

% reachable(V): V is reachable from the start.
reachable(Src) :-
    start(Src).

reachable(Dst) :-
    start(Src),
    reachable_from(Src,Dst).

% The end must be reachable from the start.
:- end(Src), not reachable(Src).

% No included edge may leave an unreachable vertex.
:- included(Src,_,_), not reachable(Src).

% Respect the ordering constraints.
:- before(X,Y), reachable(Y), not reachable_from(X,Y).
:- after(X,Y), reachable(Y), not reachable_from(Y,X).

% cost(X): X is the total cost of the included edges.
cost(X) :- X = #sum{ C,Src,Dst : included(Src,Dst,C) }.

% Solver directives

% Minimize cost; break ties on the number of included edges.
#minimize{ C@2 : cost(C) }.
#minimize{ 1@1,Src,Dst,C : included(Src,Dst,C) }.

#show included/3.
#show start/1.
#show end/1.
