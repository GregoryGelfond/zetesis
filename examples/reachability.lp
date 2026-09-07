% The first candidate derives the complete positive closure without grounding.
edge(1,2).
edge(2,3).
reach(X,Y) :- edge(X,Y).
reach(X,Z) :- reach(X,Y), edge(Y,Z).
