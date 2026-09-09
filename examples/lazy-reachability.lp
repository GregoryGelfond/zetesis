% Frozen gate choices select two different positive reachability closures.
edge(1,2). edge(2,3).
start(1) :- not blocked.
blocked :- not start(1).
reach(X) :- start(X).
reach(Y) :- reach(X), edge(X,Y).
