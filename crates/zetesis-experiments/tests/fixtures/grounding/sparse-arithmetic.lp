% Synthetic sparse-arithmetic control: one successor expression per edge.
vertex(0..63).
edge(X,X+1) :- vertex(X), X<63.
reach(0).
reach(Y) :- reach(X), edge(X,Y).
